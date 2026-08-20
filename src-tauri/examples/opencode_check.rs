//! Проверка обнаружения экземпляров OpenCode и их сессий.
//! Использование: cargo run --example opencode_check

fn main() {
    let instances = voicebridge_lib::modules::opencode::discover_instances();
    println!("найдено экземпляров: {}", instances.len());
    for inst in &instances {
        println!("== {} (port {}) ==", inst.name, inst.port);
        for s in &inst.sessions {
            println!("   - {} | {}", s.title, s.id);
        }
    }
}
