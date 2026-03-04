use log::info;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::menu::{AboutMetadata, CheckMenuItemBuilder, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

pub(crate) fn load_icon(buffer: &[u8]) -> Icon {
    let image = image::load_from_memory(buffer) // open(path)
        .expect("Failed to open icon bin")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    tray_icon::Icon::from_rgba(rgba.clone(), width, height).expect("Failed to open icon")
}

pub(crate) fn load_icons() -> (tray_icon::menu::Icon, tray_icon::Icon, tray_icon::Icon) {
    let (icon_rgba, icon_width, icon_height) = {
        let buffer = include_bytes!("../resources/aoer_logo_2018.png");
        let image = image::load_from_memory(buffer) // open(path)
            .expect("Failed to open icon bin")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let about_icon = tray_icon::menu::Icon::from_rgba(icon_rgba.clone(), icon_width, icon_height)
        .expect("Failed to open icon");

    let buffer = include_bytes!("../resources/ledfx_logo_active.png");
    let enabled_icon = load_icon(buffer);
    let buffer = include_bytes!("../resources/ledfx_logo_inactive.png");
    let disabled_icon = load_icon(buffer);

    (about_icon, enabled_icon, disabled_icon)
}

pub(crate) fn launch_taskbar_icon(enabled_send: tokio::sync::broadcast::Sender<bool>) -> String {
    let exit_menu_id: Arc<Mutex<String>> = Arc::new(Mutex::new("UNSET".to_string()));
    let exit_menu_id_return = exit_menu_id.clone();

    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        let (about_icon, icon_enabled, icon_disabled) = load_icons();
        let tray_menu = Menu::new();
        let quit_item: MenuItem = MenuItem::new("E&xit", true, None);
        let enabled_item = CheckMenuItemBuilder::new()
            .checked(true)
            .enabled(true)
            .text("Enabled")
            .build();

        tray_menu
            .append_items(&[
                &PredefinedMenuItem::about(
                    Some("About"),
                    Some(AboutMetadata {
                        name: Some("aoer-wled-doppler".to_string()),
                        copyright: Some("Copyright ArmyOfEvilRobots".to_string()),
                        version: option_env!("CARGO_PKG_VERSION")
                            .map(|version| version.to_string()),
                        icon: Some(about_icon),
                        ..Default::default()
                    }),
                ),
                &PredefinedMenuItem::separator(),
                &enabled_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
            .expect("Unexpected failure building tray menu...");
        if let Ok(mut locked_exit_menu_id) = exit_menu_id.lock() {
            *locked_exit_menu_id = quit_item.id().0.clone();
        }

        gtk::init().expect("Failed to initialize GTK for taskbar icon.");
        gtk::glib::functions::set_application_name("LedFx-Trigger");
        let the_tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_icon(icon_enabled.clone())
            .with_id("LedFx-Trigger")
            .with_title("LedFx-Trigger")
            .with_tooltip("LedFx-Trigger")
            .build()
            .unwrap();

        let mut enabled_state = true;
        let _quit_state = true;
        loop {
            gtk::main_iteration_do(false);

            if enabled_item.is_checked() != enabled_state {
                info!(
                    "Telling my service side that the enabled state is: {}",
                    enabled_item.is_checked()
                );
                enabled_send
                    .send(enabled_item.is_checked())
                    .expect("Oh darn. My broadcast socket died!");
                enabled_state = enabled_item.is_checked();
                if enabled_state {
                    the_tray_icon
                        .set_icon(Some(icon_enabled.clone()))
                        .expect("Failed to change icon for task_icon.");
                } else {
                    the_tray_icon
                        .set_icon(Some(icon_disabled.clone()))
                        .expect("Failed to change icon for task_icon.");
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    loop {
        if exit_menu_id_return
            .lock()
            .expect("Unable to unlock menu id for quit to see if it's set...")
            .as_str()
            != "UNSET"
        {
            break;
        }
    }
    exit_menu_id_return
        .lock()
        .expect("Unable to unlock menu id for quit.")
        .clone()
}
