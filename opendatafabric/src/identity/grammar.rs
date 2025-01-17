// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub struct Grammar;

/// See: https://github.com/kamu-data/open-data-fabric/blob/master/open-data-fabric.md#dataset-identity
impl Grammar {
    fn match_zero_or_many(
        s: &str,
        matcher: impl Fn(&str) -> Option<(&str, &str)>,
    ) -> Option<(&str, &str)> {
        let mut len = 0;
        let mut tail = s;
        loop {
            match matcher(tail) {
                Some((head, ntail)) => {
                    len += head.len();
                    tail = ntail;
                }
                _ => break Some((&s[0..len], &s[len..s.len()])),
            }
        }
    }

    fn match_char(s: &str, c: char) -> Option<(&str, &str)> {
        if s.len() > 0 && s.as_bytes()[0] == (c as u8) {
            Some((&s[0..1], &s[1..s.len()]))
        } else {
            None
        }
    }

    fn match_str<'a, 'b>(s: &'a str, prefix: &'b str) -> Option<(&'a str, &'a str)> {
        if s.starts_with(prefix) {
            Some((&s[0..prefix.len()], &s[prefix.len()..]))
        } else {
            None
        }
    }

    fn match_alphanums(s: &str) -> Option<(&str, &str)> {
        let alnums = s.bytes().take_while(|b| b.is_ascii_alphanumeric()).count();

        if alnums == 0 {
            None
        } else {
            Some((&s[0..alnums], &s[alnums..]))
        }
    }

    fn match_predicate(s: &str, pred: impl FnMut(&u8) -> bool) -> Option<(&str, &str)> {
        let matched = s.bytes().take_while(pred).count();
        if matched == 0 {
            None
        } else {
            Some((&s[0..matched], &s[matched..]))
        }
    }

    #[allow(dead_code)]
    // Multibase = [a-zA-Z0-9+/=]+
    fn match_multibase(s: &str) -> Option<(&str, &str)> {
        Self::match_predicate(s, |b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'/' | b'=' => true,
            _ => false,
        })
    }

    fn match_scheme(s: &str) -> Option<(&str, &str)> {
        let (h, t) = Self::match_predicate(s, |b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => true,
            _ => false,
        })?;

        let (hh, tt) = Self::match_zero_or_many(t, |s| {
            let (_, t) = Self::match_char(s, '+')?;
            let (h, tt) = Self::match_predicate(t, |b| match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => true,
                _ => false,
            })?;
            Some((&s[0..h.len() + 1], tt))
        })?;

        Some((&s[0..h.len() + hh.len()], tt))
    }

    pub fn match_url(s: &str) -> Option<(&str, &str)> {
        let (scheme, t) = Self::match_scheme(s)?;
        let (_, _) = Self::match_str(t, "://")?;
        Some((scheme, s))
    }

    #[allow(dead_code)]
    // DatasetID = "did:odf:" Multibase
    pub fn match_dataset_id(s: &str) -> Option<(&str, &str)> {
        let (h, t) = Self::match_str(s, "did:odf:")?;
        let (hh, tt) = Self::match_multibase(t)?;
        Some((&s[..h.len() + hh.len()], tt))
    }

    // Subdomain = [a-zA-Z0-9]+ ("-" [a-zA-Z0-9]+)*
    fn match_subdomain(s: &str) -> Option<(&str, &str)> {
        let (h, t) = Self::match_alphanums(s)?;

        let (hh, tt) = Self::match_zero_or_many(t, |s| {
            let (_, t) = Self::match_char(s, '-')?;
            let (h, tt) = Self::match_alphanums(t)?;
            Some((&s[0..h.len() + 1], tt))
        })?;

        Some((&s[0..h.len() + hh.len()], tt))
    }

    // Hostname = Subdomain ("." Subdomain)*
    pub fn match_hostname(s: &str) -> Option<(&str, &str)> {
        let (h, t) = Self::match_subdomain(s)?;

        let (hh, tt) = Self::match_zero_or_many(t, |s| {
            let (_, t) = Self::match_char(s, '.')?;
            let (h, tt) = Self::match_subdomain(t)?;
            Some((&s[0..h.len() + 1], tt))
        })?;

        Some((&s[0..h.len() + hh.len()], tt))
    }

    // DatasetName = Hostname
    pub fn match_dataset_name(s: &str) -> Option<(&str, &str)> {
        Self::match_hostname(s)
    }

    // AccountName = Subdomain
    pub fn match_account_name(s: &str) -> Option<(&str, &str)> {
        Self::match_subdomain(s)
    }

    // RepositoryName = Hostname
    pub fn match_repository_name(s: &str) -> Option<(&str, &str)> {
        Self::match_hostname(s)
    }

    // TenantDatasetName = (AccountName "/")? DatasetName
    pub fn match_dataset_name_with_owner(s: &str) -> Option<(Option<&str>, &str, &str)> {
        match s.split_once('/') {
            None => {
                let (ds, tail) = Self::match_dataset_name(s)?;
                Some((None, ds, tail))
            }
            Some((acc, ds)) => match Self::match_account_name(acc) {
                Some((acc, "")) => {
                    let (ds, tail) = Self::match_dataset_name(ds)?;
                    Some((Some(acc), ds, tail))
                }
                _ => None,
            },
        }
    }

    // RemoteDatasetName = RepositoryName "/" (AccountName "/")? DatasetName
    pub fn match_remote_dataset_name(s: &str) -> Option<(&str, Option<&str>, &str, &str)> {
        match s.split_once('/') {
            None => None,
            Some((repo, rest)) => match Self::match_repository_name(repo) {
                Some((repo, "")) => match rest.split_once('/') {
                    Some((acc, rest)) => match Self::match_account_name(acc) {
                        Some((acc, "")) => {
                            let (ds, tail) = Self::match_dataset_name(rest)?;
                            Some((repo, Some(acc), ds, tail))
                        }
                        _ => None,
                    },
                    None => {
                        let (ds, tail) = Self::match_dataset_name(rest)?;
                        Some((repo, None, ds, tail))
                    }
                },
                _ => None,
            },
        }
    }
}
