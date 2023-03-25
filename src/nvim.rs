/// All credits goes to [nvim-send](https://github.com/alopatindev/nvim-send)
/// This is just a copy-paste of what is done in this crate.
use anyhow::Result;
use nvim_rs::{
    create::tokio::{new_path, new_tcp},
    rpc::handler::Dummy,
};

/// Send a `command` to a Neovim running instance at `server_address`.
#[tokio::main]
pub async fn nvim(server_address: &str, command: &str) -> Result<()> {
    let handler = Dummy::new();
    if let Ok((neovim, _job_handler)) = new_path(server_address, handler).await {
        neovim.input(command).await?;
        return Ok(());
    } else {
        let handler = Dummy::new();
        if let Ok((neovim, _job_handler)) = new_tcp(server_address, handler).await {
            neovim.input(command).await?;
            return Ok(());
        }
    }

    Ok(())
}
