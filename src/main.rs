/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken
/// Then we reset the cursor, drop everything holding a terminal and print the last path.
fn main() -> anyhow::Result<()> {
    fm::modes::test_lexer();
    return Ok(());
    fm::app::FM::start()?.run()?.quit()
}
