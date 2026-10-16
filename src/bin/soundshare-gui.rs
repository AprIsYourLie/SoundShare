#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};
use soundshare::{audio, config, teamspeak};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
    FillRect, GetStockObject, InvalidateRect, RedrawWindow, RoundRect, SelectObject, SetBkColor,
    SetBkMode, SetDCBrushColor, SetTextColor, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW,
    DC_BRUSH, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HFONT, HGDIOBJ, HOLLOW_BRUSH, OPAQUE,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME,
    RDW_INVALIDATE, RDW_UPDATENOW, REDRAW_WINDOW_FLAGS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    SetWindowTheme, CB_SETCUEBANNER, DRAWITEMSTRUCT, EM_SETCUEBANNER, ODS_DISABLED, ODS_SELECTED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

const ID_PROCESS: i32 = 1001;
const ID_REFRESH: i32 = 1002;
const ID_SERVER: i32 = 1003;
const ID_CHANNEL: i32 = 1004;
const ID_NICKNAME: i32 = 1005;
const ID_START: i32 = 1006;
const ID_STOP: i32 = 1007;
const ID_CHANNEL_NOTE: i32 = 1008;
const WM_SHARE_FINISHED: u32 = WM_APP + 1;
const WM_CHANNEL_RESOLVED: u32 = WM_APP + 2;
const APP_WIDTH: i32 = 720;
const APP_HEIGHT: i32 = 610;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct GuiSettings {
    server: String,
    channel_id: String,
    channel_note: String,
    nickname: String,
    server_history: Vec<String>,
    channel_history: Vec<ChannelPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ChannelPreset {
    server: String,
    channel_id: u64,
    channel_name: String,
    note: String,
}

struct ResolvedChannelMessage {
    server: String,
    channel: teamspeak::ConnectedChannel,
}

impl ChannelPreset {
    fn display_name(&self) -> String {
        let label = if self.note.trim().is_empty() {
            self.channel_name.trim()
        } else {
            self.note.trim()
        };
        if label.is_empty() {
            format!("频道 {}", self.channel_id)
        } else {
            format!("{label}  ·  ID {}", self.channel_id)
        }
    }
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            // Public builds stay neutral. Existing users retain their locally saved address.
            server: String::new(),
            channel_id: String::new(),
            channel_note: String::new(),
            nickname: "SoundShare · 伴奏".to_string(),
            server_history: Vec::new(),
            channel_history: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum StatusTone {
    Ready,
    Working,
    Success,
    Error,
}

struct AppState {
    sessions: Vec<audio::AudioSession>,
    settings: GuiSettings,
    process_combo: HWND,
    refresh_button: HWND,
    server_combo: HWND,
    channel_combo: HWND,
    channel_note_edit: HWND,
    channel_items: Vec<ChannelPreset>,
    nickname_edit: HWND,
    start_button: HWND,
    stop_button: HWND,
    title_label: HWND,
    source_heading: HWND,
    connection_heading: HWND,
    status_label: HWND,
    hint_label: HWND,
    font_title: HFONT,
    font_heading: HFONT,
    font_body: HFONT,
    font_button: HFONT,
    font_small: HFONT,
    app_icon: HICON,
    status_tone: StatusTone,
    stop: Option<Arc<AtomicBool>>,
}

impl AppState {
    fn new(app_icon: HICON) -> Self {
        Self {
            sessions: Vec::new(),
            settings: load_settings(),
            process_combo: HWND::default(),
            refresh_button: HWND::default(),
            server_combo: HWND::default(),
            channel_combo: HWND::default(),
            channel_note_edit: HWND::default(),
            channel_items: Vec::new(),
            nickname_edit: HWND::default(),
            start_button: HWND::default(),
            stop_button: HWND::default(),
            title_label: HWND::default(),
            source_heading: HWND::default(),
            connection_heading: HWND::default(),
            status_label: HWND::default(),
            hint_label: HWND::default(),
            font_title: HFONT::default(),
            font_heading: HFONT::default(),
            font_body: HFONT::default(),
            font_button: HFONT::default(),
            font_small: HFONT::default(),
            app_icon,
            status_tone: StatusTone::Ready,
            stop: None,
        }
    }
}

fn main() {
    if let Err(error) = run_gui() {
        let message = wide(&format!("SoundShare 启动失败：\n{error}"));
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(message.as_ptr()),
                w!("SoundShare"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn run_gui() -> windows::core::Result<()> {
    unsafe {
        let module = GetModuleHandleW(None)?;
        let instance = HINSTANCE(module.0);
        let class_name = w!("SoundShareMainWindow");
        let app_icon = LoadIconW(Some(instance), resource_id(1))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))?;
        let window_class = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: app_icon,
            hInstance: instance,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 as usize + 1) as *mut c_void),
            lpszClassName: class_name,
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let state = Box::new(AppState::new(app_icon));
        let state_pointer = Box::into_raw(state);
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let x = ((screen_width - APP_WIDTH) / 2).max(0);
        let y = ((screen_height - APP_HEIGHT) / 2).max(0);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let window = match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("SoundShare · TeamSpeak 共享伴奏"),
            style,
            x,
            y,
            APP_WIDTH,
            APP_HEIGHT,
            None,
            None,
            Some(instance),
            Some(state_pointer.cast()),
        ) {
            Ok(window) => window,
            Err(error) => {
                drop(Box::from_raw(state_pointer));
                return Err(error);
            }
        };

        let _ = ShowWindow(window, SW_SHOW);
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(window);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }

        let state_pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut AppState;

        match message {
            WM_CREATE if !state_pointer.is_null() => {
                if let Err(error) = create_controls(window, &mut *state_pointer) {
                    show_error(window, &format!("无法创建界面：{error}"));
                    return LRESULT(-1);
                }
                refresh_sessions(window, &mut *state_pointer);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_PAINT if !state_pointer.is_null() => {
                paint_window(window, &*state_pointer);
                return LRESULT(0);
            }
            WM_DRAWITEM if !state_pointer.is_null() => {
                let draw = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if draw_button(draw, &*state_pointer) {
                    return LRESULT(1);
                }
            }
            WM_CTLCOLORSTATIC if !state_pointer.is_null() => {
                let state = &*state_pointer;
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut c_void);
                let control = HWND(lparam.0 as *mut c_void);
                let input_control = is_text_input_control(control);
                let text_color = if control == state.title_label {
                    rgb(20, 29, 48)
                } else if control == state.hint_label {
                    rgb(103, 113, 132)
                } else if control == state.source_heading || control == state.connection_heading {
                    rgb(72, 78, 210)
                } else if control == state.status_label {
                    status_color(state.status_tone)
                } else if input_control {
                    rgb(120, 126, 138)
                } else {
                    rgb(51, 61, 80)
                };
                SetTextColor(hdc, text_color);
                if control == state.status_label {
                    SetBkMode(hdc, OPAQUE);
                    SetBkColor(hdc, rgb(237, 243, 255));
                    SetDCBrushColor(hdc, rgb(237, 243, 255));
                    let brush = GetStockObject(DC_BRUSH);
                    return LRESULT(brush.0 as isize);
                }
                if input_control {
                    let background = rgb(240, 242, 246);
                    SetBkMode(hdc, OPAQUE);
                    SetBkColor(hdc, background);
                    SetDCBrushColor(hdc, background);
                    let brush = GetStockObject(DC_BRUSH);
                    return LRESULT(brush.0 as isize);
                }
                SetBkMode(hdc, TRANSPARENT);
                let hollow = GetStockObject(HOLLOW_BRUSH);
                return LRESULT(hollow.0 as isize);
            }
            WM_COMMAND if !state_pointer.is_null() => {
                let control_id = (wparam.0 & 0xffff) as i32;
                let notification_code = ((wparam.0 >> 16) & 0xffff) as u32;
                let state = &mut *state_pointer;
                match control_id {
                    ID_REFRESH => refresh_sessions(window, state),
                    ID_START => start_sharing(window, state),
                    ID_STOP => stop_sharing(state),
                    ID_SERVER
                        if notification_code == CBN_SELCHANGE
                            || notification_code == CBN_KILLFOCUS =>
                    {
                        if notification_code == CBN_SELCHANGE {
                            apply_server_selection(state);
                        }
                        populate_channel_history(state, false);
                    }
                    ID_CHANNEL if notification_code == CBN_SELCHANGE => {
                        apply_channel_selection(state);
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_CHANNEL_RESOLVED if !state_pointer.is_null() => {
                let state = &mut *state_pointer;
                let resolved = Box::from_raw(lparam.0 as *mut ResolvedChannelMessage);
                let note = get_text(state.channel_note_edit).trim().to_string();
                state.settings.server = resolved.server.clone();
                state.settings.channel_id = resolved.channel.id.to_string();
                state.settings.channel_note = note.clone();
                remember_server(&mut state.settings, &resolved.server);
                remember_channel(
                    &mut state.settings,
                    ChannelPreset {
                        server: resolved.server.clone(),
                        channel_id: resolved.channel.id,
                        channel_name: resolved.channel.name.clone(),
                        note,
                    },
                );
                let _ = save_settings(&state.settings);
                set_status(
                    state,
                    &format!("●  正在共享到频道：{}", resolved.channel.name),
                    StatusTone::Success,
                );
                return LRESULT(0);
            }
            WM_SHARE_FINISHED if !state_pointer.is_null() => {
                let state = &mut *state_pointer;
                let result = Box::from_raw(lparam.0 as *mut String);
                state.stop = None;
                set_running_state(state, false);
                populate_server_history(state);
                populate_channel_history(state, true);
                redraw_connection_controls(state);
                if result.contains("失败") || result.contains("无法") {
                    set_status(state, &result, StatusTone::Error);
                } else {
                    set_status(state, &result, StatusTone::Success);
                }
                return LRESULT(0);
            }
            WM_CLOSE if !state_pointer.is_null() => {
                if let Some(stop) = &(*state_pointer).stop {
                    stop.store(true, Ordering::Release);
                }
                DestroyWindow(window).ok();
                return LRESULT(0);
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            WM_NCDESTROY if !state_pointer.is_null() => {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                let state = Box::from_raw(state_pointer);
                delete_font(state.font_title);
                delete_font(state.font_heading);
                delete_font(state.font_body);
                delete_font(state.font_button);
                delete_font(state.font_small);
                drop(state);
            }
            _ => {}
        }

        DefWindowProcW(window, message, wparam, lparam)
    }
}

unsafe fn create_controls(window: HWND, state: &mut AppState) -> windows::core::Result<()> {
    unsafe {
        state.font_title = create_font(26, FW_SEMIBOLD.0 as i32);
        state.font_heading = create_font(17, FW_SEMIBOLD.0 as i32);
        state.font_body = create_font(16, FW_NORMAL.0 as i32);
        state.font_button = create_font(16, FW_SEMIBOLD.0 as i32);
        state.font_small = create_font(14, FW_NORMAL.0 as i32);

        state.title_label = create_label(window, "SoundShare", 108, 37, 380, 34, state.font_title)?;

        state.source_heading = create_label(
            window,
            "01  选择音频来源",
            44,
            137,
            260,
            24,
            state.font_heading,
        )?;
        create_label(
            window,
            "正在播放声音的程序",
            44,
            169,
            220,
            22,
            state.font_small,
        )?;
        state.process_combo = create_control(
            window,
            WINDOW_EX_STYLE::default(),
            w!("COMBOBOX"),
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_VSCROLL,
            44,
            194,
            492,
            260,
            ID_PROCESS,
            state.font_body,
        )?;
        let _ = SetWindowTheme(state.process_combo, w!("Explorer"), PCWSTR::null());
        state.refresh_button = create_control(
            window,
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            "刷新列表",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            548,
            191,
            104,
            36,
            ID_REFRESH,
            state.font_button,
        )?;

        state.connection_heading = create_label(
            window,
            "02  连接 TeamSpeak",
            44,
            264,
            280,
            24,
            state.font_heading,
        )?;
        create_label(window, "服务器地址", 44, 298, 220, 22, state.font_small)?;
        state.server_combo = create_control(
            window,
            WS_EX_CLIENTEDGE,
            w!("COMBOBOX"),
            &state.settings.server,
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE((CBS_DROPDOWN | CBS_AUTOHSCROLL) as u32),
            44,
            322,
            280,
            180,
            ID_SERVER,
            state.font_body,
        )?;
        let _ = SetWindowTheme(state.server_combo, w!("Explorer"), PCWSTR::null());
        set_combo_cue_banner(state.server_combo, "例如 ts.example.com:9987");

        create_label(window, "频道", 338, 298, 150, 22, state.font_small)?;
        state.channel_combo = create_control(
            window,
            WS_EX_CLIENTEDGE,
            w!("COMBOBOX"),
            &state.settings.channel_id,
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE((CBS_DROPDOWN | CBS_AUTOHSCROLL) as u32),
            338,
            322,
            154,
            180,
            ID_CHANNEL,
            state.font_body,
        )?;
        let _ = SetWindowTheme(state.channel_combo, w!("Explorer"), PCWSTR::null());
        set_combo_cue_banner(state.channel_combo, "默认频道 / 输入 ID");

        create_label(
            window,
            "频道备注（可选）",
            506,
            298,
            146,
            22,
            state.font_small,
        )?;
        state.channel_note_edit = create_control(
            window,
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            &state.settings.channel_note,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            506,
            322,
            146,
            34,
            ID_CHANNEL_NOTE,
            state.font_body,
        )?;
        set_cue_banner(state.channel_note_edit, "例如 音乐房");

        create_label(window, "房间显示名称", 44, 368, 220, 22, state.font_small)?;
        state.nickname_edit = create_control(
            window,
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            &state.settings.nickname,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            44,
            392,
            608,
            34,
            ID_NICKNAME,
            state.font_body,
        )?;

        populate_server_history(state);
        populate_channel_history(state, true);

        state.status_label = create_label(
            window,
            "●  正在准备音频列表…",
            48,
            454,
            594,
            26,
            state.font_body,
        )?;
        state.hint_label = create_label(
            window,
            "先播放歌曲再刷新；使用耳机可以避免麦克风把伴奏重复收进去。",
            44,
            492,
            430,
            38,
            state.font_small,
        )?;

        state.stop_button = create_control(
            window,
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            "停止",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            484,
            493,
            74,
            42,
            ID_STOP,
            state.font_button,
        )?;
        state.start_button = create_control(
            window,
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            "开始共享",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            568,
            493,
            116,
            42,
            ID_START,
            state.font_button,
        )?;
        let _ = EnableWindow(state.stop_button, false);
    }
    Ok(())
}

unsafe fn paint_window(window: HWND, state: &AppState) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let hdc = BeginPaint(window, &mut paint);
        let background = CreateSolidBrush(rgb(245, 247, 252));
        let white = CreateSolidBrush(rgb(255, 255, 255));
        let card_border = CreatePen(PS_SOLID, 1, rgb(225, 230, 240));
        let status_brush = CreateSolidBrush(rgb(237, 243, 255));
        let status_pen = CreatePen(PS_SOLID, 1, rgb(216, 227, 250));

        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        FillRect(hdc, &client, background);

        let header = RECT {
            left: 0,
            top: 0,
            right: client.right,
            bottom: 112,
        };
        FillRect(hdc, &header, white);
        let accent = RECT {
            left: 0,
            top: 108,
            right: client.right,
            bottom: 112,
        };
        let accent_brush = CreateSolidBrush(rgb(78, 91, 220));
        FillRect(hdc, &accent, accent_brush);

        let old_brush = SelectObject(hdc, HGDIOBJ(white.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(card_border.0));
        let _ = RoundRect(hdc, 24, 124, 676, 242, 14, 14);
        let _ = RoundRect(hdc, 24, 250, 676, 438, 14, 14);

        SelectObject(hdc, HGDIOBJ(status_brush.0));
        SelectObject(hdc, HGDIOBJ(status_pen.0));
        let _ = RoundRect(hdc, 24, 446, 676, 486, 14, 14);

        let _ = DrawIconEx(hdc, 44, 28, state.app_icon, 52, 52, 0, None, DI_NORMAL);

        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(background.0));
        let _ = DeleteObject(HGDIOBJ(white.0));
        let _ = DeleteObject(HGDIOBJ(accent_brush.0));
        let _ = DeleteObject(HGDIOBJ(card_border.0));
        let _ = DeleteObject(HGDIOBJ(status_brush.0));
        let _ = DeleteObject(HGDIOBJ(status_pen.0));
        let _ = EndPaint(window, &paint);
    }
}

unsafe fn draw_button(draw: &DRAWITEMSTRUCT, state: &AppState) -> bool {
    let id = draw.CtlID as i32;
    if id != ID_REFRESH && id != ID_START && id != ID_STOP {
        return false;
    }

    unsafe {
        let disabled = draw.itemState.0 & ODS_DISABLED.0 != 0;
        let pressed = draw.itemState.0 & ODS_SELECTED.0 != 0;
        let (background, foreground, border, text) = match id {
            ID_START if disabled => (
                rgb(190, 197, 214),
                rgb(245, 247, 252),
                rgb(190, 197, 214),
                "开始共享",
            ),
            ID_START if pressed => (
                rgb(55, 66, 188),
                rgb(255, 255, 255),
                rgb(55, 66, 188),
                "开始共享",
            ),
            ID_START => (
                rgb(78, 91, 220),
                rgb(255, 255, 255),
                rgb(78, 91, 220),
                "开始共享",
            ),
            ID_STOP if disabled => (
                rgb(242, 244, 248),
                rgb(166, 173, 187),
                rgb(224, 228, 236),
                "停止",
            ),
            ID_STOP if pressed => (
                rgb(255, 229, 231),
                rgb(184, 46, 60),
                rgb(235, 169, 176),
                "停止",
            ),
            ID_STOP => (
                rgb(255, 241, 242),
                rgb(190, 51, 65),
                rgb(242, 196, 201),
                "停止",
            ),
            ID_REFRESH if pressed => (
                rgb(232, 235, 247),
                rgb(56, 64, 91),
                rgb(194, 201, 220),
                "刷新列表",
            ),
            _ => (
                rgb(249, 250, 253),
                rgb(56, 64, 91),
                rgb(210, 216, 230),
                "刷新列表",
            ),
        };

        let brush = CreateSolidBrush(background);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(draw.hDC, HGDIOBJ(brush.0));
        let old_pen = SelectObject(draw.hDC, HGDIOBJ(pen.0));
        let _ = RoundRect(
            draw.hDC,
            draw.rcItem.left,
            draw.rcItem.top,
            draw.rcItem.right,
            draw.rcItem.bottom,
            12,
            12,
        );

        let old_font = SelectObject(draw.hDC, HGDIOBJ(state.font_button.0));
        SetBkMode(draw.hDC, TRANSPARENT);
        SetTextColor(draw.hDC, foreground);
        let mut text = wide(text);
        let text_len = text.len() - 1;
        let mut rect = draw.rcItem;
        DrawTextW(
            draw.hDC,
            &mut text[..text_len],
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        SelectObject(draw.hDC, old_font);
        SelectObject(draw.hDC, old_brush);
        SelectObject(draw.hDC, old_pen);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
    true
}

unsafe fn create_label(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    font: HFONT,
) -> windows::core::Result<HWND> {
    unsafe {
        create_control(
            parent,
            WS_EX_TRANSPARENT,
            w!("STATIC"),
            text,
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            width,
            height,
            0,
            font,
        )
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    parent: HWND,
    extended_style: WINDOW_EX_STYLE,
    class_name: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
    font: HFONT,
) -> windows::core::Result<HWND> {
    unsafe {
        let text = wide(text);
        let module = GetModuleHandleW(None)?;
        let control = CreateWindowExW(
            extended_style,
            class_name,
            PCWSTR(text.as_ptr()),
            style,
            x,
            y,
            width,
            height,
            Some(parent),
            Some(HMENU(id as usize as *mut c_void)),
            Some(HINSTANCE(module.0)),
            None,
        )?;
        SendMessageW(
            control,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        Ok(control)
    }
}

unsafe fn create_font(size: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        )
    }
}

unsafe fn delete_font(font: HFONT) {
    if !font.0.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

unsafe fn set_cue_banner(control: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        SendMessageW(
            control,
            EM_SETCUEBANNER,
            Some(WPARAM(1)),
            Some(LPARAM(text.as_ptr() as isize)),
        );
    }
}

unsafe fn set_combo_cue_banner(control: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        SendMessageW(
            control,
            CB_SETCUEBANNER,
            Some(WPARAM(0)),
            Some(LPARAM(text.as_ptr() as isize)),
        );
    }
}

unsafe fn combo_selection(control: HWND) -> Option<usize> {
    let selected = unsafe { SendMessageW(control, CB_GETCURSEL, None, None).0 };
    (selected >= 0).then_some(selected as usize)
}

unsafe fn combo_item_text(control: HWND, index: usize) -> Option<String> {
    unsafe {
        let length = SendMessageW(control, CB_GETLBTEXTLEN, Some(WPARAM(index)), None).0;
        if length < 0 {
            return None;
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        let copied = SendMessageW(
            control,
            CB_GETLBTEXT,
            Some(WPARAM(index)),
            Some(LPARAM(buffer.as_mut_ptr() as isize)),
        )
        .0;
        (copied >= 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
    }
}

unsafe fn apply_server_selection(state: &mut AppState) {
    if let Some(text) = combo_selection(state.server_combo)
        .and_then(|index| combo_item_text(state.server_combo, index))
    {
        unsafe { set_text(state.server_combo, &text) };
    }
}

unsafe fn apply_channel_selection(state: &mut AppState) {
    let Some(index) = (unsafe { combo_selection(state.channel_combo) }) else {
        return;
    };
    if let Some(preset) = state.channel_items.get(index) {
        unsafe { set_text(state.channel_note_edit, &preset.note) };
    }
}

unsafe fn populate_server_history(state: &mut AppState) {
    let current = state.settings.server.trim().to_string();
    let mut entries = Vec::new();
    if !current.is_empty() {
        entries.push(current.clone());
    }
    for server in &state.settings.server_history {
        let server = server.trim();
        if !server.is_empty()
            && !entries
                .iter()
                .any(|entry: &String| entry.eq_ignore_ascii_case(server))
        {
            entries.push(server.to_string());
        }
    }

    unsafe {
        SendMessageW(state.server_combo, CB_RESETCONTENT, None, None);
        for server in entries {
            let server = wide(&server);
            SendMessageW(
                state.server_combo,
                CB_ADDSTRING,
                None,
                Some(LPARAM(server.as_ptr() as isize)),
            );
        }
        set_text(state.server_combo, &current);
    }
}

unsafe fn populate_channel_history(state: &mut AppState, preserve_selection: bool) {
    let server = unsafe { get_text(state.server_combo) }.trim().to_string();
    let channel_text = if preserve_selection {
        state.settings.channel_id.clone()
    } else {
        String::new()
    };
    state.channel_items = state
        .settings
        .channel_history
        .iter()
        .filter(|preset| preset.server.eq_ignore_ascii_case(&server))
        .cloned()
        .collect();

    unsafe {
        SendMessageW(state.channel_combo, CB_RESETCONTENT, None, None);
        for preset in &state.channel_items {
            let item = wide(&preset.display_name());
            SendMessageW(
                state.channel_combo,
                CB_ADDSTRING,
                None,
                Some(LPARAM(item.as_ptr() as isize)),
            );
        }

        let selected = channel_text.parse::<u64>().ok().and_then(|channel_id| {
            state
                .channel_items
                .iter()
                .position(|preset| preset.channel_id == channel_id)
        });
        if let Some(index) = selected {
            SendMessageW(state.channel_combo, CB_SETCURSEL, Some(WPARAM(index)), None);
        } else {
            set_text(state.channel_combo, &channel_text);
        }
        if !preserve_selection {
            set_text(state.channel_note_edit, "");
        }
    }
}

fn remember_server(settings: &mut GuiSettings, server: &str) {
    let server = server.trim();
    if server.is_empty() {
        return;
    }
    settings
        .server_history
        .retain(|saved| !saved.eq_ignore_ascii_case(server));
    settings.server_history.insert(0, server.to_string());
    settings.server_history.truncate(8);
}

fn remember_channel(settings: &mut GuiSettings, preset: ChannelPreset) {
    settings.channel_history.retain(|saved| {
        !saved.server.eq_ignore_ascii_case(&preset.server) || saved.channel_id != preset.channel_id
    });
    settings.channel_history.insert(0, preset);
    settings.channel_history.truncate(32);
}

unsafe fn refresh_sessions(window: HWND, state: &mut AppState) {
    unsafe {
        set_status(state, "●  正在扫描电脑中的音频程序…", StatusTone::Working);
        SendMessageW(state.process_combo, CB_RESETCONTENT, None, None);
        match audio::active_render_sessions() {
            Ok(sessions) => {
                state.sessions = sessions;
                for session in &state.sessions {
                    let item = format!(
                        "{}    ·    PID {}    ·    音量 {:.0}%",
                        session.display_name(),
                        session.process_id,
                        session.peak * 100.0
                    );
                    let item = wide(&item);
                    SendMessageW(
                        state.process_combo,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(item.as_ptr() as isize)),
                    );
                }
                if state.sessions.is_empty() {
                    set_status(
                        state,
                        "●  暂未发现声音，请先播放歌曲后再刷新",
                        StatusTone::Ready,
                    );
                } else {
                    let selected = state
                        .sessions
                        .iter()
                        .position(|session| session.peak > 0.001)
                        .unwrap_or(0);
                    SendMessageW(
                        state.process_combo,
                        CB_SETCURSEL,
                        Some(WPARAM(selected)),
                        None,
                    );
                    set_status(
                        state,
                        &format!(
                            "●  已发现 {} 个音频程序，可以开始共享",
                            state.sessions.len()
                        ),
                        StatusTone::Ready,
                    );
                }
            }
            Err(error) => {
                state.sessions.clear();
                show_error(window, &format!("扫描音频程序失败：{error}"));
                set_status(state, "●  音频程序扫描失败", StatusTone::Error);
            }
        }
    }
}

unsafe fn start_sharing(window: HWND, state: &mut AppState) {
    unsafe {
        if state.stop.is_some() {
            return;
        }

        let selected = SendMessageW(state.process_combo, CB_GETCURSEL, None, None).0;
        if selected < 0 || selected as usize >= state.sessions.len() {
            show_error(window, "请先选择一个正在播放声音的程序。");
            return;
        }

        let server = get_text(state.server_combo).trim().to_string();
        let selected_channel = combo_selection(state.channel_combo);
        let channel_text = get_text(state.channel_combo).trim().to_string();
        let channel_note = get_text(state.channel_note_edit).trim().to_string();
        let nickname = get_text(state.nickname_edit).trim().to_string();
        if server.is_empty() {
            show_error(window, "请填写你的 TeamSpeak 服务器地址。");
            return;
        }
        let channel_id = if let Some(index) = selected_channel {
            state
                .channel_items
                .get(index)
                .map(|preset| preset.channel_id)
        } else {
            if channel_text.is_empty() {
                None
            } else {
                match channel_text.parse::<u64>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        show_error(
                            window,
                            "请选择历史频道、输入数字频道 ID，或留空进入默认频道。",
                        );
                        return;
                    }
                }
            }
        };
        let nickname = if nickname.is_empty() {
            "SoundShare · 伴奏".to_string()
        } else {
            nickname
        };

        state.settings.server = server.clone();
        state.settings.channel_id = channel_id.map(|id| id.to_string()).unwrap_or_default();
        state.settings.channel_note = channel_note;
        state.settings.nickname = nickname.clone();
        let _ = save_settings(&state.settings);

        let identity_path = match config::identity_path() {
            Ok(path) => path,
            Err(error) => {
                show_error(window, &format!("无法准备本地身份：{error}"));
                return;
            }
        };
        let process_id = state.sessions[selected as usize].process_id;
        let process_name = state.sessions[selected as usize].display_name().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        state.stop = Some(Arc::clone(&stop));
        set_running_state(state, true);
        set_status(
            state,
            &format!("●  正在连接并共享 {process_name}…"),
            StatusTone::Working,
        );

        let window_value = window.0 as isize;
        let resolved_server = server.clone();
        thread::spawn(move || {
            let settings = teamspeak::ConnectionSettings {
                server_address: server,
                nickname,
                channel_id,
                identity_path,
            };
            let resolved_window_value = window_value;
            let on_connected: teamspeak::ConnectedCallback = Box::new(move |channel| {
                let message = ResolvedChannelMessage {
                    server: resolved_server,
                    channel,
                };
                let message_pointer = Box::into_raw(Box::new(message));
                let target = HWND(resolved_window_value as *mut c_void);
                if PostMessageW(
                    Some(target),
                    WM_CHANNEL_RESOLVED,
                    WPARAM(0),
                    LPARAM(message_pointer as isize),
                )
                .is_err()
                {
                    drop(Box::from_raw(message_pointer));
                }
            });
            let result = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime
                    .block_on(teamspeak::share_application_controlled(
                        settings,
                        process_id,
                        stop,
                        Some(on_connected),
                    ))
                    .map(|_| "●  共享已安全停止".to_string())
                    .unwrap_or_else(|error| format!("●  共享失败 — {error}")),
                Err(error) => format!("●  无法启动后台服务 — {error}"),
            };
            let result_pointer = Box::into_raw(Box::new(result));
            let target = HWND(window_value as *mut c_void);
            if PostMessageW(
                Some(target),
                WM_SHARE_FINISHED,
                WPARAM(0),
                LPARAM(result_pointer as isize),
            )
            .is_err()
            {
                drop(Box::from_raw(result_pointer));
            }
        });
    }
}

unsafe fn stop_sharing(state: &mut AppState) {
    if let Some(stop) = &state.stop {
        stop.store(true, Ordering::Release);
        unsafe {
            let _ = EnableWindow(state.stop_button, false);
            set_status(state, "●  正在停止共享…", StatusTone::Working);
        }
    }
}

unsafe fn set_running_state(state: &AppState, running: bool) {
    unsafe {
        let _ = EnableWindow(state.process_combo, !running);
        let _ = EnableWindow(state.refresh_button, !running);
        let _ = EnableWindow(state.server_combo, !running);
        let _ = EnableWindow(state.channel_combo, !running);
        let _ = EnableWindow(state.channel_note_edit, !running);
        let _ = EnableWindow(state.nickname_edit, !running);
        let _ = EnableWindow(state.start_button, !running);
        let _ = EnableWindow(state.stop_button, running);
        redraw_controls(&[
            state.process_combo,
            state.refresh_button,
            state.server_combo,
            state.channel_combo,
            state.channel_note_edit,
            state.nickname_edit,
            state.start_button,
            state.stop_button,
        ]);
    }
}

unsafe fn redraw_connection_controls(state: &AppState) {
    unsafe {
        redraw_controls(&[
            state.server_combo,
            state.channel_combo,
            state.channel_note_edit,
            state.nickname_edit,
        ]);
    }
}

unsafe fn redraw_controls(controls: &[HWND]) {
    let flags = REDRAW_WINDOW_FLAGS(
        RDW_INVALIDATE.0 | RDW_ERASE.0 | RDW_FRAME.0 | RDW_ALLCHILDREN.0 | RDW_UPDATENOW.0,
    );
    unsafe {
        for control in controls {
            let _ = RedrawWindow(Some(*control), None, None, flags);
        }
    }
}

unsafe fn set_status(state: &mut AppState, text: &str, tone: StatusTone) {
    state.status_tone = tone;
    set_text(state.status_label, text);
    unsafe {
        let _ = InvalidateRect(Some(state.status_label), None, true);
    }
}

fn status_color(tone: StatusTone) -> COLORREF {
    match tone {
        StatusTone::Ready => rgb(59, 72, 105),
        StatusTone::Working => rgb(70, 82, 204),
        StatusTone::Success => rgb(31, 132, 91),
        StatusTone::Error => rgb(190, 51, 65),
    }
}

unsafe fn get_text(control: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(control).max(0) as usize;
        let mut buffer = vec![0_u16; length + 1];
        let read = GetWindowTextW(control, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..read])
    }
}

unsafe fn set_text(control: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
    }
}

unsafe fn is_text_input_control(control: HWND) -> bool {
    let mut class_name = [0_u16; 32];
    let length = unsafe { GetClassNameW(control, &mut class_name) }.max(0) as usize;
    matches!(
        String::from_utf16_lossy(&class_name[..length])
            .to_ascii_lowercase()
            .as_str(),
        "edit" | "combobox"
    )
}

unsafe fn show_error(window: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        MessageBoxW(
            Some(window),
            PCWSTR(text.as_ptr()),
            w!("SoundShare"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn settings_path() -> Option<PathBuf> {
    config::application_data_dir()
        .ok()
        .map(|directory| directory.join("settings.json"))
}

fn load_settings() -> GuiSettings {
    settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &GuiSettings) -> Result<(), Box<dyn std::error::Error>> {
    let path = settings_path().ok_or("LOCALAPPDATA is unavailable")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(settings)?)?;
    Ok(())
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF(red as u32 | ((green as u32) << 8) | ((blue as u32) << 16))
}

#[allow(clippy::manual_dangling_ptr)]
fn resource_id(id: u16) -> PCWSTR {
    // Win32's MAKEINTRESOURCE stores a numeric resource ID in the pointer value.
    PCWSTR(id as usize as *const u16)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
