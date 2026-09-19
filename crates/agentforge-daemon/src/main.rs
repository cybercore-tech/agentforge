//! `forged` daemon entry point.
//!
//! P0-M001 provides identity only. The orchestration loop is introduced by later milestones.

fn main() {
    if std::env::args()
        .skip(1)
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!(
            "{} daemon {}",
            agentforge_core::PRODUCT_NAME,
            agentforge_core::version()
        );
        return;
    }

    println!("forged: orchestration loop not implemented yet");
}
