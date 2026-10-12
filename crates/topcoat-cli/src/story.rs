//! Serves the story dashboard: the application's stories, and none of the
//! application's own routes.
//!
//! The command runs the development server's build, watch, and reload loop
//! around a build of the application made with the `design-story` feature,
//! which serves the dashboard in place of the application's router.

use clap::{Args, Subcommand};

use crate::{
    common::cargo::{BuildFlags, BuildOpts},
    dev,
};

/// The cargo feature that makes the built application serve the dashboard.
const DASHBOARD_FEATURE: &str = "topcoat/design-story";

#[derive(Args)]
pub struct StoryCommand {
    #[command(subcommand)]
    command: StorySubcommand,
}

#[derive(Subcommand)]
enum StorySubcommand {
    /// Build and serve the dashboard, watching for changes
    Run {
        #[command(flatten)]
        build: BuildFlags,
    },
}

impl StoryCommand {
    pub async fn run(self) {
        let StorySubcommand::Run { build } = self.command;
        let mut opts: BuildOpts = build.into();
        // The dashboard replaces the application's router in this build.
        opts.features.push(DASHBOARD_FEATURE.to_owned());
        dev::resolve_bin(&mut opts).await;
        dev::run(opts, "story dashboard").await;
    }
}
