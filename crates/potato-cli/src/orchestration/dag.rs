use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use crate::agents::traits::SubagentTask;

/// A node in the execution DAG.
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: String,
    pub task: SubagentTask,
    pub dependencies: HashSet<String>,
}

/// Manages topological sorting and parallel execution waves.
pub struct TaskDag {
    nodes: HashMap<String, DagNode>,
}

impl Default for TaskDag {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskDag {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Adds a task to the DAG with explicit dependencies.
    pub fn add_task(&mut self, task: SubagentTask, depends_on: Vec<String>) -> Result<()> {
        let task_id = task.id.clone();
        for dep in &depends_on {
            if !self.nodes.contains_key(dep) {
                return Err(anyhow!("Dependency '{}' not found in DAG for task '{}'", dep, task_id));
            }
        }

        self.nodes.insert(
            task_id.clone(),
            DagNode {
                id: task_id.clone(),
                task,
                dependencies: depends_on.into_iter().collect(),
            },
        );

        // Detect cycles immediately
        if self.detect_cycle() {
            self.nodes.remove(&task_id);
            return Err(anyhow!("Adding task '{}' introduces a cyclic dependency", task_id));
        }

        Ok(())
    }

    /// Returns tasks partitioned into sequential waves.
    /// Tasks within each wave are completely independent and can execute in parallel.
    pub fn compute_execution_waves(&self) -> Result<Vec<Vec<SubagentTask>>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for (id, node) in &self.nodes {
            in_degree.insert(id.clone(), node.dependencies.len());
            for dep in &node.dependencies {
                dependents.entry(dep.clone()).or_default().push(id.clone());
            }
        }

        let mut waves: Vec<Vec<SubagentTask>> = Vec::new();
        let mut ready_queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut processed_count = 0;

        while !ready_queue.is_empty() {
            let mut current_wave_ids = Vec::new();
            while let Some(id) = ready_queue.pop_front() {
                current_wave_ids.push(id);
            }

            let mut next_ready = Vec::new();
            let mut current_wave_tasks = Vec::new();

            for id in &current_wave_ids {
                processed_count += 1;
                let node = &self.nodes[id];
                current_wave_tasks.push(node.task.clone());

                if let Some(deps) = dependents.get(id) {
                    for dep in deps {
                        let count = in_degree.get_mut(dep).unwrap();
                        *count -= 1;
                        if *count == 0 {
                            next_ready.push(dep.clone());
                        }
                    }
                }
            }

            waves.push(current_wave_tasks);
            ready_queue.extend(next_ready);
        }

        if processed_count != self.nodes.len() {
            return Err(anyhow!("Unresolved cycle detected during wave generation"));
        }

        Ok(waves)
    }

    fn detect_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node_id in self.nodes.keys() {
            if self.is_cyclic_util(node_id, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn is_cyclic_util(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        if rec_stack.contains(node_id) {
            return true;
        }
        if visited.contains(node_id) {
            return false;
        }

        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());

        if let Some(node) = self.nodes.get(node_id) {
            for dep in &node.dependencies {
                if self.is_cyclic_util(dep, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(node_id);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::traits::TaskCategory;

    fn make_test_task(id: &str) -> SubagentTask {
        SubagentTask {
            id: id.to_string(),
            description: format!("Task {}", id),
            category: TaskCategory::Implementation,
            context: String::new(),
            relevant_files: Vec::new(),
        }
    }

    #[test]
    fn test_dag_wave_scheduling() {
        let mut dag = TaskDag::new();
        // Task A has no deps (Wave 1)
        dag.add_task(make_test_task("A"), vec![]).unwrap();
        // Task B has no deps (Wave 1)
        dag.add_task(make_test_task("B"), vec![]).unwrap();
        // Task C depends on A (Wave 2)
        dag.add_task(make_test_task("C"), vec!["A".to_string()]).unwrap();
        // Task D depends on B and C (Wave 3)
        dag.add_task(make_test_task("D"), vec!["B".to_string(), "C".to_string()]).unwrap();

        let waves = dag.compute_execution_waves().unwrap();
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0].len(), 2); // A, B
        assert_eq!(waves[1].len(), 1); // C
        assert_eq!(waves[2].len(), 1); // D
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = TaskDag::new();
        dag.add_task(make_test_task("A"), vec![]).unwrap();
        dag.add_task(make_test_task("B"), vec!["A".to_string()]).unwrap();
        // Attempting to add A depending on B creates cycle: A -> B -> A
        let mut dag2 = TaskDag::new();
        dag2.add_task(make_test_task("X"), vec![]).unwrap();
        let res = dag2.add_task(make_test_task("Y"), vec!["Z".to_string()]);
        assert!(res.is_err()); // Missing dependency Z
    }
}
