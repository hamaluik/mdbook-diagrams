mod cli;
use cli::Commands;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_diagrams::DiagramsPreprocessor;
use semver::{Version, VersionReq};

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = cli::cli();
    let preprocessor = DiagramsPreprocessor;

    // handle renderer checking
    if let Some(Commands::Supports { renderer }) = cli.command {
        if preprocessor.supports_renderer(&renderer) {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }

    // now actually process
    let (ctx, book) = CmdPreprocessor::parse_input(std::io::stdin())
        .map_err(|e| eyre!("Failed to parse stdin: {e}"))?;

    let book_version =
        Version::parse(&ctx.mdbook_version).wrap_err("Failed to parse incoming mdbook version")?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)
        .wrap_err("Failed to parse embedded mdbook version")?;
    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The diagrams preprocessor was built against version {} of mdbook, \
            but we're being called from version {}. This may not work.",
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = preprocessor
        .run(&ctx, book)
        .map_err(|e| eyre!("Failed to run preprocessor: {e}"))?;
    serde_json::to_writer(std::io::stdout(), &processed_book)
        .wrap_err("Failed to serialize processed book to JSON")?;
    Ok(())
}
