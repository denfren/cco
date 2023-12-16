mod cli;

use cco::value::Value;

fn main() {
    use clap::Parser;
    let cli = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("CCO_LOG"))
        .with_writer(std::io::stderr)
        .init();

    for new_path in cli.directory.iter() {
        match new_path.canonicalize() {
            Err(e) => {
                eprintln!(
                    "Failed to resolve path for -C/--directory {}\n{}",
                    new_path.display(),
                    e
                );
                std::process::exit(1);
            }
            Ok(cwd) => {
                if let Err(err) = std::env::set_current_dir(&cwd) {
                    eprintln!("Failed to set work directory to {}\n{}", cwd.display(), err,);
                    std::process::exit(1);
                }

                tracing::info!(directory=%cwd.display(), "Changed working directory");
            }
        }
    }

    let command_result = match cli.command {
        cli::Command::Evaluate(out_cli) => evaluate(out_cli),
        cli::Command::Dev(dev_cli) => dev(dev_cli),
    };

    if let Err(e) = command_result {
        for error in e.chain() {
            eprintln!("{error}")
        }
    }
}

pub fn evaluate(cli: cli::EvaluateCommand) -> anyhow::Result<()> {
    let documents = load(&cli.input)?;
    let documents = cco::cco_document::CcoDocument::new(&documents)?;

    let expr: hcl_edit::expr::Expression = cli.expression.parse()?;
    let value = documents.evaluate_in_context(expr.into())?;

    output(&cli.output, &value)?;
    Ok(())
}

fn load(input: &cli::InputArgs) -> anyhow::Result<cco::hcl_documents::HclDocuments> {
    if !input.workdir && input.files.is_empty() && input.directories.is_empty() {
        let stdin = std::io::read_to_string(std::io::stdin())?;
        let body = hcl_edit::parser::parse_body(&stdin)?;
        return Ok(body.into());
    }

    let mut documents = cco::hcl_documents::HclDocuments::default();

    if input.workdir {
        documents.load_directory(&std::env::current_dir()?)?;
    }

    for file_path in &input.files {
        documents.load_file(&file_path)?;
    }

    for dir_path in &input.directories {
        documents.load_directory(dir_path)?;
    }

    anyhow::ensure!(documents.source_count() > 0, "No files loaded");

    Ok(documents)
}

fn output(output: &cli::OutputArgs, value: &Value) -> anyhow::Result<()> {
    match output.format {
        cli::OutputFormat::Yaml => serde_yaml::to_writer(std::io::stdout(), value)?,
        cli::OutputFormat::Json => serde_json::to_writer_pretty(std::io::stdout(), value)?,
    };

    Ok(())
}

/// (cco-)developer utilities
///
/// A quick way to expose internal structures for debugging purposes
pub fn dev(cli: cli::DevCommand) -> anyhow::Result<()> {
    use cli::DevSubCommand::*;

    let mut documents = cco::hcl_documents::HclDocuments::default();
    documents.load_directory(&std::env::current_dir()?)?;

    let cco_document = cco::cco_document::CcoDocument::new(&documents).unwrap();

    match cli.command {
        Documents => println!("{documents:#?}"),
        Hcl => println!("{cco_document:#?}"),
    }

    Ok(())
}
