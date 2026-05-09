use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    Jwt,
    ApiKey,
}

/// The authenticated caller, derived once per request by the `Principal`
/// extractor. `org_id` is authoritative — handlers MUST scope queries to
/// `principal.org_id` and ignore any client-supplied org reference.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub kind: PrincipalKind,
}
