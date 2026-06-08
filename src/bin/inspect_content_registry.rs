use alpnest::content::ContentRegistry;

fn main() -> std::io::Result<()> {
    let registry = ContentRegistry::load_default()?;

    println!("contents: {}", registry.contents.len());

    for content in &registry.contents {
        println!(
            "- {} [{:?}] panels={} body={:?}",
            content.title,
            content.content_type,
            content.panels.len(),
            content.body_path
        );

        for panel in &content.panels {
            println!(
                "  - {} sections={} synthetic={}",
                panel.title,
                panel.sections.len(),
                panel.synthetic
            );

            for section in &panel.sections {
                println!("    - {} -> {}", section.title, section.body_path.display());
            }
        }
    }

    Ok(())
}
