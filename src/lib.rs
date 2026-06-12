pub mod config;
pub mod dictionary;
pub mod formats;
pub mod lucky;
pub mod query;
pub mod registry;
pub mod settings;
pub mod util;

#[cfg(test)]
mod tests {
    use crate::formats::mdict::MdxDictionary;
    use crate::dictionary::Dictionary;

    #[test]
    fn dump_wordnet_same() {
        let mdx = "/home/mohamad/.config/dicto/dicts/wordnet/WordNet.mdx";
        let dict = MdxDictionary::new(mdx);
        match dict.lookup("same") {
            Some(html) => println!("{html}"),
            None => eprintln!("word 'same' not found"),
        }
    }

    #[test]
    fn dump_mwaled_very() {
        let mdx = "/home/mohamad/.config/dicto/dicts/mwaled/mwaled.mdx";
        let dict = MdxDictionary::new(mdx);
        match dict.lookup("very") {
            Some(html) => println!("{html}"),
            None => eprintln!("word 'very' not found"),
        }
    }
}
