//! Проверка управления проектами OpenCode (list/start/stop).
//! Использование: cargo run --example projects_check

fn main() {
    let projects = voicebridge_lib::modules::opencode::list_projects();
    println!("проектов: {}", projects.len());
    for p in &projects {
        println!(
            "- {} | {} | running={} | port={}",
            p.name, p.worktree, p.running, p.port
        );
    }

    // Тест запуска/остановки на проекте, который не является текущим ("assistant").
    let Some(target) = projects.iter().find(|p| p.name != "assistant") else {
        println!("нет подходящего проекта для теста");
        return;
    };

    println!("\n== тест start/stop: {} ==", target.worktree);
    match voicebridge_lib::modules::opencode::start_project(&target.worktree) {
        Ok(()) => println!("start_project: ok"),
        Err(e) => println!("start_project: ошибка {e}"),
    }

    std::thread::sleep(std::time::Duration::from_secs(3));

    let after = voicebridge_lib::modules::opencode::list_projects();
    if let Some(p) = after.iter().find(|x| x.worktree == target.worktree) {
        println!("running после старта: {}", p.running);
    }

    match voicebridge_lib::modules::opencode::stop_project(&target.worktree) {
        Ok(()) => println!("stop_project: ok"),
        Err(e) => println!("stop_project: ошибка {e}"),
    }

    std::thread::sleep(std::time::Duration::from_secs(1));
    let after2 = voicebridge_lib::modules::opencode::list_projects();
    if let Some(p) = after2.iter().find(|x| x.worktree == target.worktree) {
        println!("running после остановки: {}", p.running);
    }
}
