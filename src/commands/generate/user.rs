use std::collections::HashMap;
use std::fmt::Display;

use dialoguer::console::Style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::FuzzySelect;
use serde_json::Value;

use super::state::TmpTarget;
use crate::channel::ChannelConfig;

pub fn create_valid_tmp_target_fn<'a>(
    channel_types: &'a [String],
    ser_types: &'a [String],
) -> impl for<'r, 's> Fn(&'r TmpTarget<'s>) -> bool + 'a {
    |ch: &TmpTarget| {
        ch.possible_channels
            .iter()
            .any(|c| channel_types.contains(c))
            && ch
                .possible_serializations
                .iter()
                .any(|c| ser_types.contains(c))
    }
}

pub fn get_if_only_one<T, I: Iterator<Item = T>>(mut iter: I) -> Option<T> {
    iter.next()
        .and_then(|v| if iter.next().is_some() { None } else { Some(v) })
}

pub fn ask_channel_config<'a>(
    id: &str,
    channel_types: &[String],
    ser_types: &[String],
    open_channels: &mut Vec<TmpTarget<'a>>,
    channel_options: &mut HashMap<String, Vec<Value>>,
    automatic: bool,
) -> Option<(ChannelConfig, Option<TmpTarget<'a>>)> {
    let is_valid_tmp_target =
        create_valid_tmp_target_fn(channel_types, ser_types);

    let options = open_channels
        .iter()
        .filter(|&x| is_valid_tmp_target(x))
        .collect::<Vec<_>>();

    // Collect indicies of options with the same name
    let automatic_options =
        options.iter().enumerate().flat_map(|(index, option)| {
            if option.name == id {
                Some(index)
            } else {
                None
            }
        });

    let automatic_option = get_if_only_one(automatic_options);

    let n = if let (true, Some(n)) = (automatic, automatic_option) {
        let chapter_style = Style::new().bold().bright();
        println!("Linking with {}", chapter_style.apply_to(&options[n]));
        n
    } else {
        ask_user_for(
            &format!("'{}' wants a channel, options:", id),
            &options,
            true,
        )
    };

    // If a target is chosen (n < options.len()) then we need to determine the channel types and
    // serialization that are both possible for the current processor and that target
    let (target, types, sers) = {
        if n >= options.len() {
            (None, channel_types.to_owned(), ser_types.to_owned())
        } else {
            let target = open_channels.remove(n);

            let types: Vec<String> = target
                .possible_channels
                .iter()
                .filter(|c| channel_types.contains(c))
                .cloned()
                .collect();

            let sers: Vec<String> = target
                .possible_serializations
                .iter()
                .filter(|c| ser_types.contains(c))
                .cloned()
                .collect();

            (Some(target), types, sers)
        }
    };

    let (config, ty) = ask_user_for_channel(&types, channel_options, automatic);
    let ser = ask_user_for_serialization(&sers);

    Some((ChannelConfig::new(ty.to_string(), ser, config), target))
}

pub fn ask_user_for_serialization<S: Display>(options: &[S]) -> String {
    let ser_index = ask_user_for("What serialization?", options, false);

    options[ser_index].to_string()
}

pub fn ask_user_for_channel<'a>(
    types: &'a [String],
    channel_options: &mut HashMap<String, Vec<Value>>,
    automatic: bool,
) -> (Value, &'a String) {
    let ty_index = ask_user_for("Choose channel type", types, false);
    let ty = &types[ty_index];

    let options = channel_options.get_mut(ty).unwrap();

    if automatic {
        let out = options.remove(0);
        let type_style = Style::new().italic();
        println!("Chosen channel config: {}", type_style.apply_to(&out));
        return (out, ty);
    }

    let channel_index = ask_user_for("Choose channel config", options, false);

    (options.remove(channel_index), ty)
}

pub fn ask_until_ready<T, E, F: FnMut() -> Result<T, E>>(mut f: F) -> T {
    loop {
        if let Ok(x) = f() {
            break x;
        }
    }
}

pub fn ask_user_for<T: std::fmt::Display>(
    name: &str,
    things: &'_ [T],
    allow_other: bool,
) -> usize {
    let theme = ColorfulTheme::default();
    let mut item = FuzzySelect::with_theme(&theme);

    item.items(things).with_prompt(name).default(0);

    if allow_other {
        item.item("Other");
    }

    loop {
        if let Ok(output) = item.interact() {
            break output;
        }
    }
}
