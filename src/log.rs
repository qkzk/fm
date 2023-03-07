use log4rs;

use crate::constant_strings_paths::LOG_CONFIG_PATH;
use crate::fm_error::FmResult;

pub fn set_loggers() -> FmResult<()> {
    log4rs::init_file(
        shellexpand::tilde(LOG_CONFIG_PATH).as_ref(),
        Default::default(),
    )?;
    Ok(())
}
