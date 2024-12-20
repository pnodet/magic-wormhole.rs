//! Wordlist generation and wormhole code utilities
use rand::{rngs::OsRng, seq::SliceRandom};
use serde_json::{self, Value};
use std::fmt;

use super::Password;

/// Represents a list of words used to generate and complete wormhole codes.
/// A wormhole code is a sequence of words used for secure communication or identification.
#[derive(PartialEq)]
pub struct Wordlist {
    /// Number of words in a wormhole code
    num_words: usize,
    /// Odd and even wordlist
    words: Vec<Vec<String>>,
}

impl fmt::Debug for Wordlist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Wordlist ( {}, lots of words...)", self.num_words)
    }
}

impl Wordlist {
    #[cfg(test)]
    #[doc(hidden)]
    pub fn new(num_words: usize, words: Vec<Vec<String>>) -> Wordlist {
        Wordlist { num_words, words }
    }

    /// Completes a wormhole code
    ///
    /// Completion can be done either with fuzzy search (approximate string matching)
    /// or simple `starts_with` matching.
    pub fn get_completions(&self, prefix: &str) -> Vec<String> {
        let words = self.get_wordlist(prefix);

        let (prefix_without_last, last_partial) = prefix.rsplit_once('-').unwrap_or(("", prefix));

        #[cfg(feature = "fuzzy-complete")]
        let matches = self.fuzzy_complete(last_partial, words);
        #[cfg(not(feature = "fuzzy-complete"))]
        let matches = self.normal_complete(last_partial, words);

        matches
            .into_iter()
            .map(|word| {
                let mut completion = String::new();
                completion.push_str(prefix_without_last);
                if !prefix_without_last.is_empty() {
                    completion.push('-');
                }
                completion.push_str(&word);
                completion
            })
            .collect()
    }

    fn get_wordlist(&self, prefix: &str) -> &Vec<String> {
        let count_dashes = prefix.matches('-').count();
        let index = 1 - (count_dashes % 2);
        &self.words[index]
    }

    #[cfg(feature = "fuzzy-complete")]
    fn fuzzy_complete(&self, partial: &str, words: &[String]) -> Vec<String> {
        // We use Jaro-Winkler algorithm because it emphasizes the beginning of a word
        use fuzzt::algorithms::JaroWinkler;

        let words = words.iter().map(|w| w.as_str()).collect::<Vec<&str>>();

        fuzzt::get_top_n(partial, &words, None, None, None, Some(&JaroWinkler))
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[allow(unused)]
    fn normal_complete(&self, partial: &str, words: &[String]) -> Vec<String> {
        words
            .iter()
            .filter(|word| !partial.is_empty() && word.starts_with(partial))
            .cloned()
            .collect()
    }

    /// Choose wormhole code word
    pub fn choose_words(&self) -> Password {
        let mut rng = OsRng;
        let components: Vec<String> = self
            .words
            .iter()
            .cycle()
            .take(self.num_words)
            .map(|words| words.choose(&mut rng).unwrap().to_string())
            .collect();
        #[allow(unsafe_code)]
        unsafe {
            Password::new_unchecked(components.join("-"))
        }
    }

    #[cfg(feature = "entropy")]
    pub(crate) fn into_words(self) -> impl Iterator<Item = String> {
        self.words.into_iter().flatten()
    }

    /// Construct Wordlist struct with given number of words in a wormhole code
    pub fn default_wordlist(num_words: usize) -> Wordlist {
        Wordlist {
            num_words,
            words: load_pgpwords(),
        }
    }
}

fn load_pgpwords() -> Vec<Vec<String>> {
    let raw_words_value: Value = serde_json::from_str(include_str!("pgpwords.json")).unwrap();
    let raw_words = raw_words_value.as_object().unwrap();
    let mut even_words: Vec<String> = Vec::with_capacity(256);
    even_words.resize(256, String::from(""));
    let mut odd_words: Vec<String> = Vec::with_capacity(256);
    odd_words.resize(256, String::from(""));
    for (index_str, values) in raw_words.iter() {
        let index = u8::from_str_radix(index_str, 16).unwrap() as usize;
        even_words[index] = values
            .get(1)
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase()
            .to_string();
        odd_words[index] = values
            .get(0)
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase()
            .to_string();
    }

    vec![even_words, odd_words]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_load_words() {
        let w = load_pgpwords();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0][0], "adroitness");
        assert_eq!(w[1][0], "aardvark");
        assert_eq!(w[0][255], "yucatan");
        assert_eq!(w[1][255], "zulu");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_default_wordlist() {
        let d = Wordlist::default_wordlist(2);
        assert_eq!(d.words.len(), 2);
        assert_eq!(d.words[0][0], "adroitness");
        assert_eq!(d.words[1][0], "aardvark");
        assert_eq!(d.words[0][255], "yucatan");
        assert_eq!(d.words[1][255], "zulu");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_get_wordlist() {
        let list = Wordlist::default_wordlist(2);
        assert_eq!(list.words.len(), 2);
        assert_eq!(list.get_wordlist("22-"), &list.words[0]);
        assert_eq!(list.get_wordlist("22-dictator-"), &list.words[1]);
    }

    fn vec_strs(all: &str) -> Vec<&str> {
        all.split_whitespace()
            .map(|s| if s == "." { "" } else { s })
            .collect()
    }

    fn vec_strings(all: &str) -> Vec<String> {
        vec_strs(all).iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_completion() {
        let words: Vec<Vec<String>> = vec![
            vec_strings("purple green yellow"),
            vec_strings("sausages seltzer snobol"),
        ];

        let w = Wordlist::new(2, words);
        assert_eq!(w.get_completions(""), Vec::<String>::new());
        assert_eq!(w.get_completions("9"), Vec::<String>::new());
        assert_eq!(w.get_completions("seltz"), vec!["seltzer"]);
        assert_eq!(w.get_completions("sausages-yello"), vec!["sausages-yellow"]);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_choose_words() {
        let few_words: Vec<Vec<String>> = vec![vec_strings("purple"), vec_strings("sausages")];

        let w = Wordlist::new(2, few_words.clone());
        assert_eq!(w.choose_words().as_ref(), "purple-sausages");
        let w = Wordlist::new(3, few_words.clone());
        assert_eq!(w.choose_words().as_ref(), "purple-sausages-purple");
        let w = Wordlist::new(4, few_words);
        assert_eq!(w.choose_words().as_ref(), "purple-sausages-purple-sausages");
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_choose_words_matches_completion() {
        let few_words: Vec<Vec<String>> = vec![vec_strings("purple"), vec_strings("sausages")];

        let w = Wordlist::new(2, few_words.clone());
        assert_eq!(w.choose_words().as_ref(), "purple-sausages");

        // Check if odd and even wordlist are correctly selected
        assert_eq!(
            w.get_completions("1-purple-sausages").first().unwrap(),
            &format!("1-{}", w.choose_words().as_ref())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_choose_more_words() {
        let more_words = vec![vec_strings("purple yellow"), vec_strings("sausages")];

        let expected2 = vec_strs("purple-sausages yellow-sausages");
        let expected3 = vec![
            "purple-sausages-purple",
            "yellow-sausages-purple",
            "purple-sausages-yellow",
            "yellow-sausages-yellow",
        ];

        let w = Wordlist::new(2, more_words.clone());
        for _ in 0..20 {
            assert!(expected2.contains(&w.choose_words().as_ref()));
        }

        let w = Wordlist::new(3, more_words);
        for _ in 0..20 {
            assert!(expected3.contains(&w.choose_words().as_ref()));
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg(feature = "fuzzy-complete")]
    fn test_completion_fuzzy() {
        let wl = Wordlist::default_wordlist(2);
        let list = wl.get_wordlist("22-");

        assert_eq!(
            wl.fuzzy_complete("bzili", list).first().unwrap(),
            "brazilian"
        );
        assert_eq!(
            wl.fuzzy_complete("carvan", list).first().unwrap(),
            "caravan"
        );
        assert_ne!(
            wl.fuzzy_complete("choking", list).first().unwrap(),
            "choking"
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_completion_normal() {
        let wl = Wordlist::default_wordlist(2);
        let list = wl.get_wordlist("22-");

        assert_eq!(
            wl.normal_complete("braz", list).first().unwrap(),
            "brazilian"
        );
        assert_eq!(wl.normal_complete("cara", list).first().unwrap(), "caravan");
        assert!(wl.normal_complete("cravan", list).is_empty());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_wormhole_code_normal_completions() {
        let list = Wordlist::default_wordlist(2);

        assert_eq!(
            list.get_completions("22-compo").first().unwrap(),
            "22-component"
        );
        assert_eq!(
            list.get_completions("22-component-check").first().unwrap(),
            "22-component-checkup"
        );
        assert_ne!(list.get_completions("22-troj"), ["trojan"]);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg(feature = "fuzzy-complete")]
    fn test_wormhole_code_fuzzy_completions() {
        let list = Wordlist::default_wordlist(2);

        assert_eq!(list.get_completions("22"), Vec::<String>::new());
        assert_eq!(list.get_completions("22-"), Vec::<String>::new());
        assert_ne!(list.get_completions("22-trj"), ["trojan"]);

        assert_eq!(
            list.get_completions("22-udau").first().unwrap(),
            "22-undaunted"
        );

        assert_eq!(
            list.get_completions("22-undua").first().unwrap(),
            "22-undaunted"
        );

        assert_eq!(
            list.get_completions("22-undaunted-usht").first().unwrap(),
            "22-undaunted-upshot"
        );
    }
}
