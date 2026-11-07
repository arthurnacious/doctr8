// Re-embed migrations (sqlx::migrate!) whenever a migration file changes.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
