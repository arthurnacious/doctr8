use std::sync::Arc;

use axum::extract::FromRef;
use sqlx::PgPool;

use crate::{
    common::jwt::JwtKeys,
    config::Config,
    modules::{
        access::{
            auth::service::AuthService, roles::service::RolesService,
            tenants::service::TenantsService, users::service::UsersService,
        },
        people::{
            accounts::service::AccountsService, memberships::service::MembershipsService,
            patients::service::PatientsService, persons::service::PersonsService,
        },
    },
};

/// Application container. Handlers pull exactly what they need via
/// `State<RolesService>` etc. (`FromRef`), much like NestJS constructor injection.
#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    pub jwt: JwtKeys,
    pub auth: AuthService,
    pub tenants: TenantsService,
    pub users: UsersService,
    pub roles: RolesService,
    pub persons: PersonsService,
    pub memberships: MembershipsService,
    pub accounts: AccountsService,
    pub patients: PatientsService,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let config = Arc::new(config);
        let jwt = JwtKeys::new(
            config.jwt_secret.as_bytes(),
            &config.jwt_issuer,
            config.access_token_ttl_secs,
        );
        Self {
            auth: AuthService::new(pool.clone(), jwt.clone(), config.clone()),
            tenants: TenantsService::new(pool.clone()),
            users: UsersService::new(pool.clone()),
            roles: RolesService::new(pool.clone()),
            persons: PersonsService::new(pool.clone()),
            memberships: MembershipsService::new(pool.clone()),
            accounts: AccountsService::new(pool.clone()),
            patients: PatientsService::new(pool),
            jwt,
            config,
        }
    }
}
