// Recompila quando muda uma migração (o `sqlx::migrate!` as embute no binário).
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
