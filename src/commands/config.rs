use {
    crate::{models::Config, schema::configs},
    diesel::prelude::*,
};

fn list() -> seahorse::Command {
    seahorse::Command::new("list")
        .description("List all project configuration file(s) and their shorthands")
        .usage("nova config list")
        .action(|_| {
            let configs = configs::dsl::configs
                .load::<Config>(&mut crate::create_connection())
                .unwrap();

            let mut table = prettytable::Table::new();
            table.set_titles(prettytable::row!["Shorthand", "Filename", "Content Length"]);

            for config in configs {
                table.add_row(prettytable::row![
                    config.shorthand,
                    config.filename,
                    config.content.len()
                ]);
            }

            table.printstd();
        })
}

fn clone() -> seahorse::Command {
    seahorse::Command::new("clone")
        .description("Clone project configuration file(s) to the current working directory")
        .usage("nova config clone [...shorthands]")
        .action(|context| {
            for shorthand in &context.args {
                let config = match configs::dsl::configs
                    .filter(configs::shorthand.eq(shorthand))
                    .first::<Config>(&mut crate::create_connection())
                {
                    Ok(config) => config,
                    Err(_) => {
                        println!("Unknown config shorthand: {}", shorthand);
                        continue;
                    }
                };

                match std::fs::write(std::path::PathBuf::from(&config.filename), config.content) {
                    Ok(_) => println!("Wrote to file: {}", config.filename),
                    Err(err) => {
                        println!("Unable to write file: {}", config.filename);
                        println!("Error: {}", err);
                    }
                }
            }
        })
}

fn vim() -> seahorse::Command {
    seahorse::Command::new("vim")
        .description("View a project configuration file in Vim")
        .usage("nova config vim [filename]")
        .action(|context| {
            let filename = match context.args.first() {
                Some(filename) => filename,
                None => {
                    println!("Please provide a filename");
                    return;
                }
            };

            let config = match configs::dsl::configs
                .filter(configs::filename.eq(filename))
                .first::<Config>(&mut crate::create_connection())
            {
                Ok(config) => config,
                Err(_) => {
                    println!("Unknown config filename: {}", filename);
                    return;
                }
            };

            let path = std::path::PathBuf::from(format!("{}.temp", &config.filename));
            if let Err(err) = std::fs::write(&path, &config.content) {
                println!("Unable to temp file: {}", config.filename);
                println!("Error: {}", err);
                return;
            }

            let mut child = match std::process::Command::new("/usr/bin/vim")
                .arg(&path.to_str().unwrap())
                .spawn()
            {
                Ok(child) => child,
                Err(err) => {
                    println!("Failed to run editor");
                    println!("Error: {}", err);
                    return;
                }
            };

            match child.wait() {
                Ok(_) => {}
                Err(err) => {
                    println!("Editor returned a non-zero status");
                    println!("Error: {}", err);
                    return;
                }
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(err) => {
                    println!("Unable to read temp file: {}", config.filename);
                    println!("Error: {}", err);
                    return;
                }
            };

            if let Err(err) = std::fs::remove_file(path) {
                println!("Unable to remove temp file: {}", config.filename);
                println!("Error: {}", err);
                return;
            }

            if &content == &config.content {
                println!("No changes made to file: {}", config.filename);
                return;
            }

            match diesel::update(configs::dsl::configs)
                .filter(configs::filename.eq(&filename))
                .set(configs::content.eq(&content))
                .execute(&mut crate::create_connection())
            {
                Ok(_) => {
                    println!("Updated config: {}", &config.filename);
                }
                Err(err) => {
                    println!("Unable to update config: {}", &config.filename);
                    println!("Error: {}", err);
                    return;
                }
            }
        })
}

fn add() -> seahorse::Command {
    seahorse::Command::new("add")
        .description("Add a new configuration file, uses file content if the file exists")
        .usage("nova config add [filename] [shorthand]")
        .action(|context| {
            let filename = match context.args.first() {
                Some(filename) => filename,
                None => {
                    println!("Please provide a filename, then a shorthand");
                    return;
                }
            };

            let shorthand = match context.args.get(1) {
                Some(shorthand) => shorthand,
                None => {
                    println!("Please provide a shorthand");
                    return;
                }
            };

            let content = match std::fs::read_to_string(std::path::PathBuf::from(&filename)) {
                Ok(content) => content,
                Err(_) => {
                    println!("Could not read file data: {filename}");
                    String::new()
                }
            };

            match configs::dsl::configs
                .filter(configs::filename.eq(filename))
                .count()
                .get_result::<i64>(&mut crate::create_connection())
            {
                Ok(configs) => {
                    if configs != 0 {
                        println!("Filename already exists");
                        return;
                    }
                }
                Err(err) => {
                    println!("Unable to fetch configs");
                    println!("Error: {}", err);
                    return;
                }
            };

            match configs::dsl::configs
                .filter(configs::shorthand.eq(shorthand))
                .count()
                .get_result::<i64>(&mut crate::create_connection())
            {
                Ok(configs) => {
                    if configs != 0 {
                        println!("Shorthand already exists");
                        return;
                    }
                }
                Err(err) => {
                    println!("Unable to fetch configs");
                    println!("Error: {}", err);
                    return;
                }
            };

            let config = Config {
                filename: filename.to_string(),
                shorthand: shorthand.to_string(),
                content,
            };

            match diesel::insert_into(configs::dsl::configs)
                .values(&config)
                .execute(&mut crate::create_connection())
            {
                Ok(_) => {
                    println!("Config created: {filename} ({shorthand})")
                }
                Err(err) => {
                    println!("Unable to store new config: {filename} ({shorthand})");
                    println!("Error: {err}");
                }
            }
        })
}

fn remove() -> seahorse::Command {
    seahorse::Command::new("remove")
        .description("Remove a configuration file")
        .usage("nova config remove [filename]")
        .action(|context| {
            let filename = match context.args.first() {
                Some(filename) => filename,
                None => {
                    println!("Please provide a filename");
                    return;
                }
            };

            match diesel::delete(configs::dsl::configs)
                .filter(configs::filename.eq(&filename))
                .execute(&mut crate::create_connection())
            {
                Ok(deleted) => {
                    if deleted == 0 {
                        println!("Unknown config filename: {filename}");
                    } else {
                        println!("Config removed: {filename}");
                    }
                }
                Err(err) => {
                    println!("Unable to delete config");
                    println!("Error: {}", err);
                }
            };
        })
}

pub fn config() -> seahorse::Command {
    seahorse::Command::new("config")
        .description("Manage reusable project configuration files")
        .command(list())
        .command(clone())
        .command(vim())
        .command(add())
        .command(remove())
        .action(|context| context.help())
}
