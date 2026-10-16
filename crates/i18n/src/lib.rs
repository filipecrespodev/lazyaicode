//! Idioma da interface. Os textos ficam em `locales/<idioma>.json`, com
//! chaves `<área>.<chave>` (`thread.land`, `gitWorking.commit`…). Uma chave
//! que falta no idioma escolhido cai no inglês; uma que falta em todos volta
//! como a própria chave (visível, nunca um pânico).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// `(id, nome nativo, locale para formatação)`.
pub const LOCALES: [(&str, &str, &str); 4] =
    [("en", "English", "en-US"), ("pt", "Português", "pt-BR"), ("es", "Español", "es-ES"), ("zh", "中文", "zh-CN")];

pub const DEFAULT_LOCALE: &str = "en";

type Dict = HashMap<String, String>;

fn parse(src: &str) -> Dict {
    serde_json::from_str::<HashMap<String, serde_json::Value>>(src)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
        .collect()
}

fn dicts() -> &'static HashMap<&'static str, Dict> {
    static D: OnceLock<HashMap<&'static str, Dict>> = OnceLock::new();
    D.get_or_init(|| {
        HashMap::from([
            ("en", parse(include_str!("../locales/en.json"))),
            ("pt", parse(include_str!("../locales/pt.json"))),
            ("es", parse(include_str!("../locales/es.json"))),
            ("zh", parse(include_str!("../locales/zh.json"))),
        ])
    })
}

static CURRENT: RwLock<&'static str> = RwLock::new(DEFAULT_LOCALE);

/// Troca o idioma; um id desconhecido vira o inglês.
pub fn set_locale(id: &str) {
    let next = LOCALES.iter().find(|l| l.0 == id).map_or(DEFAULT_LOCALE, |l| l.0);
    if let Ok(mut c) = CURRENT.write() {
        *c = next;
    }
}

pub fn locale() -> &'static str {
    CURRENT.read().map(|c| *c).unwrap_or(DEFAULT_LOCALE)
}

/// Texto da chave no idioma atual.
pub fn t(key: &str) -> String {
    lookup(locale(), key)
}

/// Texto com `{nome}` substituído pelos valores dados.
pub fn tf(key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = t(key);
    for (k, v) in vars {
        s = s.replace(&format!("{{{k}}}"), &v.to_string());
    }
    s
}

pub fn lookup(locale: &str, key: &str) -> String {
    let d = dicts();
    d.get(locale)
        .and_then(|m| m.get(key))
        .or_else(|| d.get(DEFAULT_LOCALE).and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// Todas as chaves de um idioma (testes de cobertura e `xtask i18n`).
pub fn keys(locale: &str) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = dicts().get(locale).map(|m| m.keys().map(String::as_str).collect()).unwrap_or_default();
    v.sort_unstable();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_has_every_english_key() {
        let en = keys("en");
        assert!(en.len() > 1000);
        for (id, _, _) in LOCALES {
            let missing: Vec<_> = en.iter().filter(|k| !keys(id).contains(k)).collect();
            assert!(missing.is_empty(), "{id} sem {missing:?}");
        }
    }

    #[test]
    fn falls_back_to_english_then_to_the_key() {
        assert_eq!(lookup("xx", "thread.land"), lookup("en", "thread.land"));
        assert_eq!(lookup("pt", "nao.existe"), "nao.existe");
    }

    #[test]
    fn fills_placeholders() {
        set_locale("en");
        let s = tf("time.minutes", &[("n", &5)]);
        assert!(s.contains('5') && !s.contains("{n}"), "{s}");
    }
}
