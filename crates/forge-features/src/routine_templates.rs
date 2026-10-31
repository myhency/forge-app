use forge_domain::Schedule;
use forge_shared::tr;

use crate::routines::{FormDraft, ScheduleKind};

/// A ready-made routine: pressing it on the Routines page opens the form already filled in. The
/// folder is left for the person to choose, and nothing is saved until they press Create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub instructions: &'static str,
    pub schedule: Schedule,
}

impl RoutineTemplate {
    /// The form this template opens: the texts and the schedule, no folder.
    pub fn to_form(&self) -> FormDraft {
        let mut form = FormDraft { name: self.name.to_string(), description: self.description.to_string(), instructions: self.instructions.to_string(), ..FormDraft::new() };
        match &self.schedule {
            Schedule::Manual => form.kind = ScheduleKind::Manual,
            Schedule::Hourly => form.kind = ScheduleKind::Hourly,
            Schedule::Daily { hour, minute } => (form.kind, form.hour, form.minute) = (ScheduleKind::Daily, *hour, *minute),
            Schedule::Weekdays { hour, minute } => (form.kind, form.hour, form.minute) = (ScheduleKind::Weekdays, *hour, *minute),
            Schedule::Weekly { day, hour, minute } => (form.kind, form.day, form.hour, form.minute) = (ScheduleKind::Weekly, *day, *hour, *minute),
            Schedule::Cron { expression } => (form.kind, form.cron) = (ScheduleKind::Custom, expression.clone()),
        }
        form
    }
}

/// The templates the page lists. They work on the folder of the routine alone (no connector, no
/// account): the desktop app's templates for mail, calendar, issues and pull requests need services
/// Forge does not have. The minutes are odd on purpose, so routines of different people do not all
/// start on the hour.
pub fn routine_templates() -> Vec<RoutineTemplate> {
    vec![
        RoutineTemplate {
            name: tr!("Daily briefing"),
            description: tr!("Summarises what changed in the folder since yesterday and what is still open."),
            instructions: tr!("Look at the git history and the working tree of this folder since yesterday. Write a short briefing: 1. What changed (commits and uncommitted work). 2. What looks unfinished (TODO comments, failing or skipped tests, open branches). 3. What to do first today. Keep it to a few bullet points. Skip a section that has nothing to report."),
            schedule: Schedule::Weekdays { hour: 9, minute: 18 },
        },
        RoutineTemplate {
            name: tr!("Review yesterday's commits"),
            description: tr!("Reads the commits of the last day and flags risky changes and missing tests."),
            instructions: tr!("Read the commits of the last 24 hours in this folder. Summarise what changed, point out risky patterns or changes that have no test, and note anything that needs a follow-up. Do not change any file."),
            schedule: Schedule::Weekdays { hour: 9, minute: 42 },
        },
        RoutineTemplate {
            name: tr!("Dependency update check"),
            description: tr!("Looks for outdated packages, security patches and breaking changes."),
            instructions: tr!("List the dependencies of this project that are outdated, and say which updates are security patches and which are likely to break something (a new major version, a removed API). Suggest an order for updating them. Do not change any file."),
            schedule: Schedule::Weekly { day: 2, hour: 3, minute: 16 },
        },
        RoutineTemplate {
            name: tr!("Flaky test finder"),
            description: tr!("Runs the tests a few times and finds the ones that pass and fail by turns."),
            instructions: tr!("Run the test suite of this project three times. Report the tests that passed in some runs and failed in others, with the error of each failure and a guess at the cause (timing, shared state, order). Do not change any file."),
            schedule: Schedule::Weekly { day: 2, hour: 0, minute: 54 },
        },
        RoutineTemplate {
            name: tr!("Release notes draft"),
            description: tr!("Drafts release notes for users from the commits since the last tag. Run it by hand."),
            instructions: tr!("Find the last git tag of this folder and read the commits since it. Draft release notes for the users of the project: group them as New, Improved and Fixed, in plain words, and leave out changes only a developer would care about."),
            schedule: Schedule::Manual,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_makes_a_form_that_needs_only_a_folder() {
        for template in routine_templates() {
            let form = template.to_form();
            assert!(form.folder.is_none(), "{}", template.name);
            // Everything but the folder is filled in: the only thing the form complains about is the folder.
            let mut with_folder = form.clone();
            with_folder.folder = Some("/work".into());
            assert_eq!(with_folder.error(), None, "{}", template.name);
            assert_eq!(form.to_draft().schedule, template.schedule, "{}", template.name);
        }
    }

    #[test]
    fn the_templates_have_distinct_names_and_texts() {
        let templates = routine_templates();
        for (index, template) in templates.iter().enumerate() {
            assert!(!template.name.trim().is_empty() && !template.description.trim().is_empty() && !template.instructions.trim().is_empty());
            assert!(templates.iter().skip(index + 1).all(|other| other.name != template.name));
        }
    }

    #[test]
    fn a_weekly_template_is_on_a_tuesday_and_a_manual_one_has_no_time() {
        let forms: Vec<FormDraft> = routine_templates().iter().map(RoutineTemplate::to_form).collect();
        assert!(forms.iter().any(|form| form.kind == ScheduleKind::Weekly && form.day == 2));
        assert!(forms.iter().any(|form| form.kind == ScheduleKind::Manual));
    }
}
