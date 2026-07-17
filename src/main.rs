struct Task {
    description: String,
    done: bool,
    id: u32,
}


fn main() {
    println!("Task manager\n");
    
    let mut all_tasks = Vec::new(); 

    add_tasks(&mut all_tasks);

    println!("All tasks:");
    for task in all_tasks.iter() {
        println!("Task: {}, ID: {}, Done: {}", task.description, task.id, task.done);
    }
}



fn add_tasks(all_tasks: &mut Vec<Task>) {
    println!("Input tasks, type \"exit\" when finished");
    let mut i = if all_tasks.is_empty() { 1 } 
    else { all_tasks[all_tasks.len()].id};
    loop {
        println!("Task number {}", i);

        let mut buffer = String::new();

        std::io::stdin().read_line(&mut buffer)
            .expect("Failed to read line");

        buffer = buffer.trim().to_string();
        if buffer == "exit" { break };
        if buffer.is_empty() { continue };

        let task_in = Task {
            description: String::from(buffer),
            done: false,
            id: i,
        };
        println!("Task: {}, ID: {}, Done: {}", task_in.description, task_in.id, task_in.done);

        i += 1;
        all_tasks.push(task_in);
    };
}