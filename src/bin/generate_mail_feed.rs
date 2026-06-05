use color_eyre::Result;

use alpnest::mail::{feed::build_mail_feed, render::render_mail_feed, store::MailStore};

fn main() -> Result<()> {
    color_eyre::install()?;

    let store = MailStore::load_default()?;
    let generated_mail_dir = store.data_home.join("generated").join("mail");
    let threads = store.build_threads();
    let feed = build_mail_feed(threads.clone(), &generated_mail_dir);

    render_mail_feed(&feed, &threads, &generated_mail_dir)?;

    println!(
        "generated {} active mail slots in {}",
        feed.slots.len(),
        generated_mail_dir.display()
    );

    Ok(())
}
