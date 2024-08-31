/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken
/// Then we reset the cursor, drop everything holding a terminal and print the last path.
fn main() -> anyhow::Result<()> {
    let mut fm = fm::app::FM::start()?;
    fm::io::google_drive()?;
    fm.run()?;
    let last_path = fm.quit()?;
    fm::common::print_on_quit(last_path);
    Ok(())
}
