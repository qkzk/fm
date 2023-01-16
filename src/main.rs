use std::sync::Arc;

use clap::Parser;
use log::info;

use fm::args::Args;
use fm::config::load_config;
use fm::constant_strings_paths::CONFIG_PATH;
use fm::event_dispatch::EventDispatcher;
use fm::fm_error::FmResult;
use fm::help::Help;
use fm::log::set_logger;
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{drop_everything, init_term, print_on_quit};

/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken and we reset the cursor.
fn main2() -> FmResult<()> {
    set_logger()?;
    info!("fm is starting");

    let config = load_config(CONFIG_PATH)?;
    info!("config loaded");
    let term = Arc::new(init_term()?);
    let event_dispatcher = EventDispatcher::new(config.binds.clone());
    let event_reader = EventReader::new(term.clone());
    let help = Help::from_keybindings(&config.binds)?.help;
    let mut display = Display::new(term.clone());
    let mut status = Status::new(Args::parse(), config, display.height()?, term.clone(), help)?;

    while let Ok(event) = event_reader.poll_event() {
        event_dispatcher.dispatch(&mut status, event)?;
        status.refresh_disks();
        display.display_all(&status)?;

        if status.selected_non_mut().must_quit() {
            break;
        };
    }

    display.show_cursor()?;
    let final_path = status.selected_path_str().to_owned();
    drop_everything(term, event_dispatcher, event_reader, status, display);
    print_on_quit(&final_path);
    info!("fm is shutting down");
    Ok(())
}

fn main() -> FmResult<()> {
    use users::{get_current_uid, get_user_by_uid};

    use fm::cryptsetup::{filter_crypto_devices_lines, get_devices, CryptoDevice, PasswordHolder};

    let ret_val = get_devices()?;
    println!("{}", ret_val);
    let output = filter_crypto_devices_lines(ret_val, "crypto");
    println!("{:?}", output);
    let mut crypto_device = CryptoDevice::from_line(&output[0])?;
    let password_holder = PasswordHolder::default()
        .with_sudo("aze")
        .with_cryptsetup("aze");
    let user = get_user_by_uid(get_current_uid())
        .ok_or_else(|| fm::fm_error::FmError::custom("username", "couldn't read username"))?;
    let username = user
        .name()
        .to_str()
        .ok_or_else(|| fm::fm_error::FmError::custom("username", "couldn't read username"))?;
    println!("{:?}", crypto_device);
    println!(
        "mounted ? {}",
        crypto_device.open_mount(&username, &password_holder)?
    );
    // println!(
    //     "unmounted ? {}",
    //     crypto_device.umount_close("quentin", &password_holder)?
    // );

    Ok(())
}
