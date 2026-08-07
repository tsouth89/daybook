//! One-shot maintenance: reformat every page in a real vault into the standing
//! -document shape, through the same renderer the app uses. Ignored by default;
//! it writes to a real vault.
//!
//!   DAYBOOK_REFORMAT_VAULT=<path> cargo test -p daybook-core \
//!     --test reformat_real_vault -- --ignored --nocapture
#[test]
#[ignore]
fn reformat() {
    let Some(v) = std::env::var_os("DAYBOOK_REFORMAT_VAULT") else {
        eprintln!("set DAYBOOK_REFORMAT_VAULT");
        return;
    };
    let v = std::path::PathBuf::from(v);
    let fmt = std::env::var("DAYBOOK_DATE_FORMAT").unwrap_or_else(|_| "MM/DD/YYYY".into());
    for m in daybook_core::vault::read_projects_config(&v) {
        match daybook_core::vault::render_entity_page(&v, &m.kind, &m.slug, &fmt) {
            Ok(()) => println!("reformatted {}/{}", m.kind, m.slug),
            Err(e) => println!("skipped {}: {e}", m.slug),
        }
    }
}
