use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use itertools::Itertools;
use num_format::{SystemLocale, ToFormattedString};
use proc_macro2::TokenTree;
use rayon::iter::{ParallelBridge, ParallelIterator};

fn get_files(file: PathBuf) -> Vec<PathBuf> {
    if file.is_dir() {
        file.read_dir()
            .expect("failed to read_dir")
            .flat_map(|file| {
                let file = file.unwrap().path();
                if file.is_dir() {
                    get_files(file)
                } else {
                    vec![file]
                }
            })
            .collect()
    } else {
        vec![file]
    }
}

fn flatten_groups(tree: TokenTree) -> Vec<TokenTree> {
    match tree {
        TokenTree::Group(group) => group
            .stream()
            .into_iter()
            .flat_map(flatten_groups)
            .collect_vec(),
        _ => {
            vec![tree]
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(num_args = 1..)]
    files: Vec<PathBuf>,
    #[arg(short, long)]
    check: bool,
    #[arg(short, long)]
    token_limit: Option<usize>,
    #[arg(short, long)]
    string_limit: Option<usize>,
    #[arg(short, long)]
    ignore_punct: bool,
}

fn main() {
    let args = Args::parse();
    let locale = SystemLocale::default().expect("failed to get system locale");

    let result = args
        .files
        .into_iter()
        .flat_map(get_files)
        .par_bridge()
        .filter(|file| file.extension().is_some_and(|ext| ext.eq("rs")))
        .map(|file| {
            let mut tokens = 0;
            let mut chars = 0;
            proc_macro2::TokenStream::from_str(
                &fs::read_to_string(file).expect("failed to read file"),
            )
            .unwrap()
            .into_iter()
            .flat_map(flatten_groups)
            .filter(|token| {
                if args.ignore_punct {
                    match token {
                        TokenTree::Group(_) | TokenTree::Literal(_) | TokenTree::Ident(_) => true,
                        TokenTree::Punct(_) => false, // Ignore punctuation . , * : etc
                    }
                } else {
                    true
                }
            })
            .for_each(|token| {
                //dbg!(&token);
                tokens += 1;
                if let TokenTree::Literal(val) = token {
                    if let Ok(val) = litrs::StringLit::try_from(val) {
                        chars += val.value().len();
                    }
                }
            });
            (tokens, chars)
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));

    println!(
        "Tokens: {} | String chars: {}",
        result.0.to_formatted_string(&locale),
        result.1.to_formatted_string(&locale)
    );

    #[allow(clippy::useless_let_if_seq)]
    if args.check {
        let mut fail = false;
        if args.token_limit.is_some_and(|limit| result.0 > limit) {
            eprintln!("Token Limit Exceeded");
            fail = true;
        }
        if args.string_limit.is_some_and(|limit| result.1 > limit) {
            eprintln!("String Char Limit Exceeded");
            fail = true;
        }
        if fail {
            std::process::exit(1);
        }
    }
}
