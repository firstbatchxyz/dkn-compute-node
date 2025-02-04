use std::collections::HashSet;

use dkn_workflows::{Model, ModelProvider};
use inquire::{MultiSelect, Select};

pub fn edit_models() -> eyre::Result<()> {
    // TODO: take this from env
    let selected_models =
        HashSet::<Model>::from_iter([Model::GPT4Turbo, Model::GPT4oMini, Model::Gemini15Flash]);

    // choose a provider
    loop {
        let Some(provider) =
            Select::new("Select a model provider:", ModelProvider::all().collect())
                .with_help_message("↑↓ to move, enter to select, type to filter, ESC to go back")
                .prompt_skippable()?
        else {
            break;
        };

        // TODO: filter out the models that are already selected
        // let existing_provider_models = selected_models
        //     .iter()
        //     .filter(|model| Provi model.provider() == provider);

        // then choose a model of that provider
        let provider_models = Model::all_with_provider(provider).collect::<Vec<_>>();
        let provider_model_selections = provider_models
            .iter()
            .enumerate()
            .filter_map(|(idx, model)| {
                if selected_models.contains(model) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let Some(ans) = MultiSelect::new(
            "Choose the models that you would like to serve:",
            provider_models,
        )
        .with_default(&provider_model_selections)
        // .with_validator(validator)
        // .with_formatter(formatter)
        .with_help_message(
            "↑↓ to move, space to select one, → to all, ← to none, type to filter, ESC to go back",
        )
        .prompt_skippable()?
        else {
            continue;
        };

        // FIXME: return
        let new_models = HashSet::<Model>::from_iter(ans);
        println!("Selected models: {:?}", new_models);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_providers() {
        edit_models().unwrap();
    }
}
