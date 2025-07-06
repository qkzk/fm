/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken
/// Then we reset the cursor, drop everything holding a terminal and print the last path.
fn main() -> anyhow::Result<()> {
    fm::app::FM::start()?.run()?.quit()
}

// // example plugin

// use crossterm::{
//     event::{self, DisableMouseCapture, EnableMouseCapture, Event},
//     execute,
//     terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
// };
// use libloading::{Library, Symbol};
// use plugin_api::{DrawContext, PluginEntryFn, PluginInfo};
// use ratatui::{backend::CrosstermBackend, Terminal};
//
// use std::io;
// use std::time::Duration;
//
// struct AppContext; // dummy for now
//
// impl DrawContext for AppContext {}
//
// fn main2() -> Result<(), Box<dyn std::error::Error>> {
//     enable_raw_mode()?;
//     let mut stdout = io::stdout();
//     execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
//     let backend = CrosstermBackend::new(stdout);
//     let mut terminal = Terminal::new(backend)?;
//
//     let lib = unsafe { Library::new("plugins/hello_world/target/release/libHello_World.so")? };
//     let entry: Symbol<PluginEntryFn> = unsafe { lib.get(b"plugin_entry")? };
//     let plugin_info: &mut PluginInfo = unsafe { &mut *entry() };
//
//     let mut ctx = AppContext;
//
//     loop {
//         terminal.draw(|f| {
//             let area = f.area();
//             (plugin_info.draw)(&mut ctx, f, area);
//         })?;
//
//         if event::poll(Duration::from_millis(200))? {
//             if let Event::Key(k) = event::read()? {
//                 if (plugin_info.on_event)(&mut ctx, k) {
//                     println!("Plugin a consommé : {:?}", k);
//                 }
//                 if k.code == crossterm::event::KeyCode::Char('q') {
//                     break;
//                 }
//             }
//         }
//     }
//
//     disable_raw_mode()?;
//     execute!(
//         terminal.backend_mut(),
//         LeaveAlternateScreen,
//         DisableMouseCapture
//     )?;
//     Ok(())
// }
