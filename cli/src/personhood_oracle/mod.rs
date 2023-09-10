/*
	Copyright 2021 Integritee AG and Supercomputing Systems AG

	Licensed under the Apache License, Version 2.0 (the "License");
	you may not use this file except in compliance with the License.
	You may obtain a copy of the License at

		http://www.apache.org/licenses/LICENSE-2.0

	Unless required by applicable law or agreed to in writing, software
	distributed under the License is distributed on an "AS IS" BASIS,
	WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
	See the License for the specific language governing permissions and
	limitations under the License.

*/

use crate::{
	personhood_oracle::{
		commands::{FetchReputationCmd, IssueNodeTemplateXtCmd, IssueNostrBadgeCmd},
		PersonhoodOracleCommand::{FetchReputation, IssueNodeTemplateXt, IssueNostrBadge},
	},
	Cli,
};
mod commands;

/// Oracle subcommands for the cli.
#[derive(Debug, clap::Subcommand)]
pub enum PersonhoodOracleCommand {
	FetchReputation(FetchReputationCmd),
	IssueNostrBadge(IssueNostrBadgeCmd),
	IssueNodeTemplateXt(IssueNodeTemplateXtCmd),
}

impl PersonhoodOracleCommand {
	pub fn run(&self, cli: &Cli) {
		match self {
			FetchReputation(fetch_reputation_cmd) => fetch_reputation_cmd.run(cli),
			IssueNostrBadge(issue_nostr_badge_cmd) => issue_nostr_badge_cmd.run(cli),
			IssueNodeTemplateXt(issue_node_template_xt_cmd) => issue_node_template_xt_cmd.run(cli),
		}
	}
}
