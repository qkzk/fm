use tuikit::error::TuikitError;

/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken
/// Then we reset the cursor, drop everything holding a terminal and print the last path.
fn main() -> anyhow::Result<()> {
    let mut fm = fm::app::FM::start()?;

    loop {
        match fm.peek_event() {
            Ok(event) => {
                fm.update(event)?;
                fm.display()?;
                if fm.must_quit() {
                    break;
                }
            }
            Err(TuikitError::Timeout(_)) => continue,
            Err(error) => {
                fm::log_info!("Error in main loop: {error}");
                break;
            }
        }
    }

    fm.quit()
}
