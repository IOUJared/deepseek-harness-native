//! Per-preset help, separate from the picker's short copy: the reference keeps a curated guide for
//! each shipped mode and shows it behind two disclosures on the card
//! (`ui-agent-preset/src/client/guide-locales.ts:2-79`). Only shipped presets have one; a custom
//! preset's own description is the whole of its help, as in the reference.

/// One shipped preset's guide: the sentence shown above the card's choices, the explanation of how
/// the mode works, and the worked examples.
pub struct Guide {
    pub id: &'static str,
    pub intro: &'static str,
    pub explanation: &'static str,
    pub usage: &'static str,
}

/// The reference's two disclosure labels (`guide-locales.ts:4-5`).
pub const MODE_DETAILS: &str = "Mode details";
pub const HOW_TO_USE: &str = "How to use";
pub const EXAMPLE_TASK: &str = "Example task";
/// The reference's copy control and its one-second confirmation (`guide-locales.ts:6-7`).
pub const COPY: &str = "Copy";
pub const COPIED: &str = "Copied";

const STANDARD: Guide = Guide {
    id: "standard",
    intro: "Choose Standard mode when starting a new task. Describe what you want to accomplish, point to the relevant files, and explain how to check the result.",
    explanation: "How it works\nThe agent calls tools directly to read and edit files, search, and run terminal commands. It includes Skills, planning, goals, subagents, workflows, and context compaction.\n\nWhen to choose it\nStart here for everyday coding, file work, and research. Standard mode can also write scripts and process files in batches. PTC changes how tool calls are organized; it is not required for batch tasks.",
    usage: "Fix a bug\n> Find out why submitting the search form twice makes the results disappear. Fix it and run the relevant tests. Explain the cause and what changed.\nExpected output: a code change, the relevant test results, and an explanation of the cause.\n\nOrganize project notes\n> Read the Markdown notes in this project. Summarize the agreed decisions and open questions, with links to the source files.\nExpected output: a summary with references that you can check against the original notes.",
};

const PTC: Guide = Guide {
    id: "ptc",
    intro: "Choose PTC mode when starting a new task. Specify the input files, processing rules, and output format. The agent writes the code.",
    explanation: "How tools are called\nPTC means Programmatic Tool Calling. In this built-in preset, the agent uses run_code to write a TypeScript program that calls tools through a generated SDK. The program can use loops, conditions, error handling, and concurrent calls where appropriate.\n\nWhat reaches the model\nTool results first reach the program, which can filter and combine them. The model receives what the program prints or returns; image results are attached separately. Nested tool calls are still recorded and remain subject to tool permissions.\n\nCompared with Standard mode\nBoth modes can handle coding and batch tasks. Standard mode exposes individual tools directly; PTC organizes tool calls in code. The current PTC preset leaves the workflow tool disabled. Speed and token use depend on the task and how the program handles its results.",
    usage: "Check a set of configuration files\n> Check all JSON files in configs/. List missing required fields and invalid values against schema.json. Save a CSV with one row per issue. Include unreadable files in the report and keep checking the rest. Leave the original files unchanged.\nExpected output: an issue summary and a CSV report. The program can repeat the same checks, handle individual failures, and collect the results.\n\nSummarize error logs\n> Analyze the log files in logs/. Group errors by service and error type. Show the ten most frequent groups and one example from each. Save the full counts to a CSV.\nExpected output: the top error groups and a complete count table. Intermediate data can be aggregated in the program before the summary reaches the model.",
};

const MINIMAL: Guide = Guide {
    id: "minimal",
    intro: "Choose Minimal mode for a new task. For a comparison, hold the model, permissions, input, and starting workspace state constant across runs.",
    explanation: "What is included\nOne persistent shell tool and a fixed system prompt. The built-in preset does not load Skills, planning, context compaction, or the standard runtime context.\n\nWhen to choose it\nUse it as a baseline for experiments and comparisons. It can still read files and execute scripts through shell commands, but offers fewer built-in ways to manage a long task. Fewer tools does not necessarily make it easier for a beginner.",
    usage: "Compare performance on a small bug fix\n> Run the tests for this project, find the cause of the failure, and make the smallest fix. Run the relevant tests again and report the result.\nRun the same task separately in Standard and Minimal modes from the same starting state. Compare task completion, tool calls, and the resulting changes. Minimal mode performs the work through terminal commands.",
};

const CORDIS: Guide = Guide {
    id: "cordis",
    intro: "Choose Creator mode for a new task. Describe the capability you want, where it should appear, and how you will verify it.",
    explanation: "What you can create\nCreator mode includes the standard task tools plus runtime inspection, persistent plugin management, and guidance for authoring Cordis plugins and agent presets. It can create a plugin that adds a capability or UI, or a preset that combines tools and prompts for a particular job.\n\nPlugins and modes\nA plugin adds capabilities to DSH, such as a tool, a service connection, or a UI entry. A mode is an agent preset that selects tools and defines how the agent works in a task. A plugin can be included in a custom preset.\n\nHow the result takes effect\nAsk the agent to install and verify the result, not just generate source code. A plugin may load immediately or require a restart, depending on what it changes. A newly created preset is selected when starting a new task.",
    usage: "Add a UI\n> Create a DSH plugin that adds a project notes entry to the sidebar. Let me browse the Markdown files in this workspace and preview a selected note. Install it and verify that the page opens.\nExpected output: an installed plugin with a working entry and preview page, plus any remaining activation steps.\n\nAdd a tool\n> Create a plugin with a tool that reads this project's test report and summarizes the failed tests. Register it and verify it with a sample report.\nExpected output: a plugin with a callable tool and a verified sample call.\n\nCreate my own mode\n> Create a \"Code review\" mode based on Standard mode. Have it prioritize potential bugs and test gaps, cite file paths and lines, and ask before modifying files. Save it as a selectable preset.\nExpected output: a custom preset for new tasks. These review instructions guide the agent; permission settings determine which actions it can execute.",
};

const GUIDES: [Guide; 4] = [STANDARD, PTC, MINIMAL, CORDIS];

/// The guide for one preset id, or `None` for a preset the reference does not curate.
pub fn guide(id: &str) -> Option<&'static Guide> {
    GUIDES.iter().find(|guide| guide.id == id)
}

#[cfg(test)]
mod tests {
    //! The guide exists for exactly the shipped presets the reference curates.
    use super::*;

    #[test]
    fn only_the_four_shipped_modes_have_a_guide() {
        for id in ["standard", "ptc", "minimal", "cordis"] {
            let guide = guide(id).unwrap_or_else(|| panic!("{id} has a guide"));
            assert!(!guide.intro.is_empty());
            assert!(!guide.explanation.is_empty());
            assert!(!guide.usage.is_empty());
            // The disclosure labels are the reference's own.
            assert_eq!(MODE_DETAILS, "Mode details");
            assert_eq!(HOW_TO_USE, "How to use");
        }
        assert!(guide("team-reviewer").is_none());
        assert!(guide("PUBLIC-custom").is_none());
    }

    #[test]
    fn each_guide_states_what_it_is_for_before_how_it_works() {
        let standard = guide("standard").expect("shipped");
        assert!(standard.intro.starts_with("Choose Standard mode"));
        assert!(standard.explanation.starts_with("How it works\n"));
        // The usage section names its example, which is what the reference shows under the
        // "Example task" heading.
        assert!(standard.usage.starts_with("Fix a bug\n"));
        assert_eq!(EXAMPLE_TASK, "Example task");
    }
}
