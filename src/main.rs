#[allow(unused_imports)]
#[macro_use]
extern crate clap;

#[allow(unused_imports)]
#[macro_use]
extern crate failure;
use failure::Error;

extern crate config;
extern crate serde;
#[allow(unused_imports)]
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;

extern crate ubackup;
use ubackup::Settings;

use std::fs::File;
use std::path::Path;

fn main() -> Result<(), Error> {
    let cli = clap_app!(uBackup =>
        (version: crate_version!())
        (author: crate_authors!("\n"))
        (about: crate_description!())
        (@arg config: -c --config +takes_value "Config file (yml)")
        (@group q =>
            (@arg quiet: -q --quiet "Quiet")
            (@arg verbose: -v --verbose "Verbose")
        )
        (@group r =>
            (@arg dryrun: -d --dryrun "Dry (don't run)")
            (@arg run: -r --run "Run")
        )
    );
    let cli: clap::ArgMatches = cli.get_matches();

    let config_file = cli.value_of("config").unwrap_or("config.yaml");
    let config_path = Path::new(config_file);

    if !config_path.exists() {
        println!("Creating default config at {}.", config_file);

        return Ok(serde_yaml::to_writer(
            File::create(config_path)?,
            &Settings::new(None)?,
        )?);
    }

    let mut settings = Settings::new(Some(config_file))?;

    if cli.is_present("quiet") {
        settings.config.quiet = true;
    }
    if cli.is_present("verbose") {
        settings.config.quiet = false;
    }

    if cli.is_present("dryrun") {
        settings.config.dryrun = true;
    }
    if cli.is_present("run") {
        settings.config.dryrun = false;
    }

    let [successes, errors, copied, skiped]: [u32; 4] = ubackup::backup(settings.clone())?;

    println!(
        "{} successes, {} errors, {} copies, {} skips",
        successes, errors, copied, skiped
    );
    Ok(())
}
