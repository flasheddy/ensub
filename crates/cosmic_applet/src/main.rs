use anyhow::Result;
use ensub_applet::{run, AppletFlags};
use ensub_sqlite::resolve_database_path;

fn main() -> Result<()> {
    run(AppletFlags {
        database_path: resolve_database_path(None)?,
    })?;
    Ok(())
}
