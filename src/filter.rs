use crate::config::AllowEntry;

/// Reasons a request may be rejected by the allow-list filter.
#[derive(Debug)]
pub enum FilterError {
    DomainNotAllowed,
    ServiceNotAllowed,
}

impl FilterError {
    pub fn message(&self) -> &'static str {
        match self {
            FilterError::DomainNotAllowed => "domain not allowed",
            FilterError::ServiceNotAllowed => "service not allowed",
        }
    }
}

/// Check whether `domain` and `service` are permitted by the allow-list.
///
/// Rules:
/// - `domain` must appear in `allow`.
/// - If `services` for that entry is non-empty, `service` must be in it.
/// - If `services` is empty, all services for that domain are allowed.
pub fn check(allow: &[AllowEntry], domain: &str, service: &str) -> Result<(), FilterError> {
    let entry = allow
        .iter()
        .find(|e| e.domain == domain)
        .ok_or(FilterError::DomainNotAllowed)?;

    if !entry.services.is_empty() && !entry.services.iter().any(|s| s == service) {
        return Err(FilterError::ServiceNotAllowed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AllowEntry;

    fn entry(domain: &str, services: &[&str]) -> AllowEntry {
        AllowEntry {
            domain: domain.to_string(),
            services: services.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn domain_not_in_list_is_rejected() {
        let allow = vec![entry("light", &["turn_on"])];
        assert!(matches!(
            check(&allow, "switch", "turn_on"),
            Err(FilterError::DomainNotAllowed)
        ));
    }

    #[test]
    fn service_not_in_list_is_rejected() {
        let allow = vec![entry("light", &["turn_on"])];
        assert!(matches!(
            check(&allow, "light", "turn_off"),
            Err(FilterError::ServiceNotAllowed)
        ));
    }

    #[test]
    fn empty_services_allows_any_service() {
        let allow = vec![entry("light", &[])];
        assert!(check(&allow, "light", "anything").is_ok());
    }

    #[test]
    fn exact_match_is_allowed() {
        let allow = vec![entry("light", &["turn_on", "turn_off"])];
        assert!(check(&allow, "light", "turn_on").is_ok());
        assert!(check(&allow, "light", "turn_off").is_ok());
    }
}
