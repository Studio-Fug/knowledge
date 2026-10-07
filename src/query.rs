use crate::{
    Error, Result,
    model::{Completeness, Rigor, ScopeKind, Summary},
    store::Cache,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Query {
    pub text: String,
    pub trust_publisher: Option<String>,
    pub include_incomplete: bool,
    pub rigor: Option<Rigor>,
    pub scope: Option<ScopeKind>,
    pub supported_claims_only: bool,
    pub exclude_superseded: bool,
    pub limit: usize,
}

#[derive(Serialize)]
pub struct Search {
    pub matches: Vec<Summary>,
    pub scanned: usize,
    pub scope: &'static str,
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| {
            !t.is_empty() && !["a", "an", "the", "for", "to", "of", "and", "method"].contains(t)
        })
        .map(|t| {
            let root = t
                .strip_suffix("ing")
                .or_else(|| t.strip_suffix('s'))
                .unwrap_or(t);
            if root.len() > 3 {
                root.strip_suffix('e').unwrap_or(root).to_owned()
            } else {
                root.to_owned()
            }
        })
        .collect()
}

fn close(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 1 || a.len().min(b.len()) < 4 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else {
            edits += 1;
            if edits > 1 {
                return false;
            }
            if a.len() >= b.len() {
                i += 1;
            }
            if b.len() >= a.len() {
                j += 1;
            }
        }
    }
    edits + (a.len() - i) + (b.len() - j) <= 1
}

pub fn search(cache: &Cache, query: &Query) -> Result<Search> {
    if query.text.len() > 8192 || query.limit > 100 {
        return Err(Error::new("invalid_query", "query exceeds limits"));
    }
    if query
        .trust_publisher
        .as_ref()
        .is_some_and(|k| !crate::canonical::is_hex(k, 32))
    {
        return Err(Error::new("invalid_key", "invalid trusted publisher"));
    }
    let wanted = tokens(&query.text);
    let addresses = cache.addresses()?;
    let mut eligible = Vec::new();
    for address in &addresses {
        let artifact = cache.get(address)?;
        let summary = artifact.summary(query.trust_publisher.as_deref(), "local".into())?;
        if !query.include_incomplete && summary.completeness != Completeness::ReportedComplete {
            continue;
        }
        if query.scope.is_some_and(|s| s != summary.subject.kind) {
            continue;
        }
        let claims: Vec<_> = artifact
            .payload
            .claims
            .iter()
            .zip(&summary.claims)
            .filter(|(_, s)| {
                (!query.supported_claims_only || s.status == "publisher_reported_support")
                    && query.rigor.is_none_or(|r| s.rigor.contains(&r))
            })
            .collect();
        let text = claims
            .iter()
            .flat_map(|(c, _)| {
                std::iter::once(c.text.as_str()).chain(c.aliases.iter().map(String::as_str))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let text = if query.supported_claims_only || query.rigor.is_some() {
            text
        } else {
            format!(
                "{} {} {}",
                artifact.payload.specification.title,
                artifact
                    .payload
                    .specification
                    .requirements
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                text
            )
        };
        let available = tokens(&text);
        if !wanted.iter().all(|w| available.iter().any(|a| close(w, a))) {
            continue;
        }
        if (query.supported_claims_only || query.rigor.is_some()) && claims.is_empty() {
            continue;
        }
        eligible.push((artifact, summary));
    }
    let superseded: BTreeSet<_> = eligible
        .iter()
        .flat_map(|(a, _)| {
            a.payload
                .predecessors
                .iter()
                .filter(|p| {
                    p.revision
                        && eligible.iter().any(|(old, summary)| {
                            summary.address == p.address
                                && old.payload.publisher == a.payload.publisher
                        })
                })
                .map(|p| p.address.clone())
        })
        .collect();
    let limit = if query.limit == 0 { 20 } else { query.limit };
    let matches = eligible
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| !query.exclude_superseded || !superseded.contains(&s.address))
        .take(limit)
        .collect();
    Ok(Search {
        matches,
        scanned: addresses.len(),
        scope: "local_cache; publisher_assertions; eligible_revisions_only",
    })
}
