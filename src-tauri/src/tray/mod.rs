use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, warn};

pub(crate) fn setup_tray(app: &tauri::App) -> Result<(), tauri::Error> {
    use tauri::image::Image;
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    use tauri::tray::TrayIconBuilder;

    let settings_item = MenuItem::with_id(app, "settings", "設定", true, None::<&str>)?;
    #[cfg(target_os = "macos")]
    let shortcut_settings = crate::config::settings::get_shortcut_settings();
    #[cfg(target_os = "macos")]
    let input_shortcut = shortcut_settings.input_shortcut;
    #[cfg(target_os = "macos")]
    let os_paste_shortcut = shortcut_settings.os_paste_shortcut;

    #[cfg(target_os = "macos")]
    let paste_last_item = MenuItem::with_id(
        app,
        "paste_last",
        "最後の文字起こしをペースト",
        true,
        Some(input_shortcut.as_str()),
    )?;

    #[cfg(target_os = "windows")]
    let paste_last_item = MenuItem::with_id(
        app,
        "paste_last",
        "最後の文字起こしをペースト",
        true,
        None::<&str>,
    )?;

    #[cfg(target_os = "macos")]
    let copy_os_dictation_item = Some(MenuItem::with_id(
        app,
        "copy_os_dictation",
        "最後のOS標準音声入力をペースト",
        true,
        Some(os_paste_shortcut.as_str()),
    )?);

    #[cfg(target_os = "windows")]
    let copy_os_dictation_item = None::<MenuItem<tauri::Wry>>;

    let mic_submenu = build_microphone_submenu(app)?;

    let lang = crate::config::settings::get_language();
    let lang_ja = MenuItem::with_id(
        app,
        "lang_ja",
        if lang == "ja" {
            "✓ 日本語"
        } else {
            "日本語"
        },
        true,
        None::<&str>,
    )?;
    let lang_en = MenuItem::with_id(
        app,
        "lang_en",
        if lang == "en" {
            "✓ English"
        } else {
            "English"
        },
        true,
        None::<&str>,
    )?;
    let lang_submenu = Submenu::with_items(app, "Language", true, &[&lang_ja, &lang_en])?;

    let current_model = crate::config::get_model();
    let model_lite = MenuItem::with_id(
        app,
        "model_lite",
        if current_model == "gemini-2.5-flash-lite" {
            "✓ gemini-2.5-flash-lite"
        } else {
            "gemini-2.5-flash-lite"
        },
        true,
        None::<&str>,
    )?;
    let model_flash = MenuItem::with_id(
        app,
        "model_flash",
        if current_model == "gemini-2.5-flash" {
            "✓ gemini-2.5-flash"
        } else {
            "gemini-2.5-flash"
        },
        true,
        None::<&str>,
    )?;
    let model_preview = MenuItem::with_id(
        app,
        "model_preview",
        if current_model == "gemini-3-flash-preview" {
            "✓ gemini-3-flash-preview"
        } else {
            "gemini-3-flash-preview"
        },
        true,
        None::<&str>,
    )?;
    let model_submenu = Submenu::with_items(
        app,
        "モデル",
        true,
        &[&model_lite, &model_flash, &model_preview],
    )?;

    let sound_mode = crate::config::settings::get_sound_mode();
    let sound_off = MenuItem::with_id(
        app,
        "sound_off",
        if sound_mode == crate::config::settings::FeedbackMode::AlwaysOff {
            "✓ 常にOFF"
        } else {
            "常にOFF"
        },
        true,
        None::<&str>,
    )?;
    let sound_on = MenuItem::with_id(
        app,
        "sound_on",
        if sound_mode == crate::config::settings::FeedbackMode::AlwaysOn {
            "✓ 常にON"
        } else {
            "常にON"
        },
        true,
        None::<&str>,
    )?;
    let sound_fullscreen = MenuItem::with_id(
        app,
        "sound_fullscreen",
        if sound_mode == crate::config::settings::FeedbackMode::FullscreenOnly {
            "✓ フルスクリーン時のみON"
        } else {
            "フルスクリーン時のみON"
        },
        true,
        None::<&str>,
    )?;
    let sound_submenu = Submenu::with_items(
        app,
        "通知音 (SE)",
        true,
        &[&sound_off, &sound_on, &sound_fullscreen],
    )?;

    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "KoeTypeを完全に終了", true, None::<&str>)?;

    let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    menu_items.push(&settings_item);
    menu_items.push(&paste_last_item);
    if let Some(item) = copy_os_dictation_item.as_ref() {
        menu_items.push(item);
    }
    menu_items.push(&separator);
    menu_items.push(&mic_submenu);
    menu_items.push(&lang_submenu);
    menu_items.push(&model_submenu);
    menu_items.push(&sound_submenu);
    menu_items.push(&separator);
    menu_items.push(&quit_item);

    let menu = Menu::with_items(app, &menu_items)?;

    #[cfg(target_os = "macos")]
    let tray_icon_bytes: &[u8] = include_bytes!("../../icons/tray-macos@2x.png");

    #[cfg(target_os = "windows")]
    let tray_icon_bytes: &[u8] = include_bytes!("../../icons/tray-windows.png");

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let tray_icon_bytes: &[u8] = include_bytes!("../../icons/tray-macos@2x.png");

    let tray_icon = image::load_from_memory(tray_icon_bytes)
        .ok()
        .map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Image::new_owned(rgba.into_raw(), w, h)
        })
        .or_else(|| app.default_window_icon().cloned());

    let mut builder = TrayIconBuilder::new();
    if let Some(icon) = tray_icon {
        builder = builder.icon(icon);
    }
    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }
    builder
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_tray_menu_event)
        .build(app)?;

    Ok(())
}

fn build_microphone_submenu(
    app: &tauri::App,
) -> Result<tauri::menu::Submenu<tauri::Wry>, tauri::Error> {
    use tauri::menu::{MenuItem, Submenu};

    let devices = crate::audio::recorder::list_input_devices();
    let selected = crate::config::settings::get_selected_microphone();

    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();

    let default_label = if selected.is_none() {
        "✓ システムデフォルト"
    } else {
        "システムデフォルト"
    };
    let default_item = MenuItem::with_id(app, "mic_default", default_label, true, None::<&str>)?;
    items.push(default_item);

    for device in devices {
        let is_selected = selected.as_ref() == Some(&device.id);
        let label = if is_selected {
            format!("✓ {}", device.name)
        } else {
            device.name.clone()
        };
        let item = MenuItem::with_id(
            app,
            &format!("mic_{}", device.id),
            &label,
            true,
            None::<&str>,
        )?;
        items.push(item);
    }

    let item_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    Submenu::with_items(app, "マイク", true, &item_refs)
}

pub(crate) fn handle_tray_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "quit" => {
            #[cfg(target_os = "macos")]
            {
                crate::overlay::shutdown_overlay(app);
            }
            app.exit(0);
        }
        "settings" => {
            let _ =
                crate::commands::open_settings_window(app.clone(), app.state::<crate::AppState>());
        }
        "paste_last" => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(text) = crate::paste::resolve_latest_llm_text(&app_handle) {
                    if let Err(e) =
                        crate::paste::paste_text_preserving_clipboard(&app_handle, &text)
                    {
                        warn!("ペーストエラー: {}", e);
                    } else {
                        debug!("最後の文字起こしをペーストしました");
                    }
                } else {
                    debug!("文字起こしの履歴がありません");
                }
            });
        }
        #[cfg(not(target_os = "windows"))]
        "copy_os_dictation" => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(text) = crate::paste::resolve_latest_os_text(&app_handle) {
                    if let Err(e) =
                        crate::paste::paste_text_preserving_clipboard(&app_handle, &text)
                    {
                        warn!("OS標準音声入力ペースト失敗: {}", e);
                    } else {
                        debug!("OS標準音声入力をペーストしました");
                    }
                } else {
                    debug!("OS標準音声入力の履歴がありません");
                }
            });
        }
        id if id.starts_with("mic_") => {
            let device_name = if id == "mic_default" {
                None
            } else {
                id.strip_prefix("mic_").map(|suffix| suffix.to_string())
            };

            if let Err(e) = crate::config::settings::set_selected_microphone(device_name.clone()) {
                warn!("マイク設定の保存エラー: {}", e);
            }
            debug!("マイクデバイスを設定しました: {:?}", device_name);
        }
        "lang_ja" => {
            if let Err(e) = crate::config::settings::set_language("ja") {
                warn!("言語設定の保存に失敗しました: {}", e);
            } else {
                debug!("言語を日本語に設定しました");
            }
        }
        "lang_en" => {
            if let Err(e) = crate::config::settings::set_language("en") {
                warn!("言語設定の保存に失敗しました: {}", e);
            } else {
                debug!("言語を英語に設定しました");
            }
        }
        "model_lite" => {
            if let Err(e) = crate::config::set_model("gemini-2.5-flash-lite") {
                warn!("モデル設定の保存に失敗しました: {}", e);
            } else {
                debug!("モデルを gemini-2.5-flash-lite に設定しました");
            }
        }
        "model_flash" => {
            if let Err(e) = crate::config::set_model("gemini-2.5-flash") {
                warn!("モデル設定の保存に失敗しました: {}", e);
            } else {
                debug!("モデルを gemini-2.5-flash に設定しました");
            }
        }
        "model_preview" => {
            if let Err(e) = crate::config::set_model("gemini-3-flash-preview") {
                warn!("モデル設定の保存に失敗しました: {}", e);
            } else {
                debug!("モデルを gemini-3-flash-preview に設定しました");
            }
        }
        "sound_off" => {
            if let Err(e) = crate::config::settings::set_sound_mode(
                crate::config::settings::FeedbackMode::AlwaysOff,
            ) {
                warn!("音声設定の保存に失敗しました: {}", e);
            } else {
                debug!("音声設定を 常にOFF に設定しました");
            }
            let _ = app.emit(
                "settings-updated",
                crate::FeedbackSettings {
                    notification_mode: crate::config::settings::get_notification_mode(),
                    sound_mode: crate::config::settings::FeedbackMode::AlwaysOff,
                },
            );
        }
        "sound_on" => {
            if let Err(e) = crate::config::settings::set_sound_mode(
                crate::config::settings::FeedbackMode::AlwaysOn,
            ) {
                warn!("音声設定の保存に失敗しました: {}", e);
            } else {
                debug!("音声設定を 常にON に設定しました");
            }
            let _ = app.emit(
                "settings-updated",
                crate::FeedbackSettings {
                    notification_mode: crate::config::settings::get_notification_mode(),
                    sound_mode: crate::config::settings::FeedbackMode::AlwaysOn,
                },
            );
        }
        "sound_fullscreen" => {
            if let Err(e) = crate::config::settings::set_sound_mode(
                crate::config::settings::FeedbackMode::FullscreenOnly,
            ) {
                warn!("音声設定の保存に失敗しました: {}", e);
            } else {
                debug!("音声設定を フルスクリーン時のみON に設定しました");
            }
            let _ = app.emit(
                "settings-updated",
                crate::FeedbackSettings {
                    notification_mode: crate::config::settings::get_notification_mode(),
                    sound_mode: crate::config::settings::FeedbackMode::FullscreenOnly,
                },
            );
        }
        _ => (),
    }
}
