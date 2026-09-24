use crate::agents::traits::TaskCategory;
use crate::engine::context::assembler::ProjectContext;

/// Builds production-grade, highly constrained system prompts customized
/// for each specialist agent and grounded in the active project context.
pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    pub fn build(category: &TaskCategory, context: &ProjectContext) -> String {
        let mut prompt = String::with_capacity(4096);

        // Tier 1: Identity & Core Mandate
        prompt.push_str(Self::identity_block(category));

        // Tier 2: Real-time Operating Environment Context
        prompt.push_str("\n\n## OPERATING ENVIRONMENT & GROUNDING\n");
        prompt.push_str(&format!("- Operating System: {}\n", context.os));
        prompt.push_str(&format!("- Architecture: {}\n", context.arch));
        prompt.push_str(&format!("- Workspace Root: {}\n", context.working_dir));
        if !context.detected_toolchains.is_empty() {
            prompt.push_str(&format!(
                "- Active Toolchains: {}\n",
                context.detected_toolchains.join(", ")
            ));
        }
        if let Some(ref branch) = context.git_branch {
            prompt.push_str(&format!("- Git Branch: {}\n", branch));
        }
        if !context.modified_files.is_empty() {
            prompt.push_str(&format!(
                "- Uncommitted Modified Files: {}\n",
                context.modified_files.join(", ")
            ));
        }

        // Tier 3: Historical Lessons & Anti-Patterns
        if !context.relevant_lessons.is_empty() {
            prompt.push_str("\n## HISTORICAL LESSONS (MANDATORY COMPLIANCE)\n");
            for lesson in &context.relevant_lessons {
                prompt.push_str(&format!("- {}\n", lesson));
            }
        }

        // Tier 3.5: User Personality & Custom Rules (POTATO.md / GEMINI.md)
        let rules = crate::engine::rules::RulesEngine::load_from(&context.working_dir);
        if !rules.is_empty() {
            prompt.push_str(&rules.formatted_prompt_block());
        }

        // Tier 4: Core Operational Directives & Safety Rules
        prompt.push_str(
            r#"
## CORE OPERATIONAL DIRECTIVES
1. THOUGHT BEFORE ACTION: Always reason through architectural implications before proposing actions.
2. SURGICAL MUTATIONS: Never rewrite entire files for small changes. Use exact line slices and unique patches.
3. ZERO PLACEHOLDERS: Never produce "TODO", "mock implementation", or ellipsis comments. All code must be complete and deployable.
4. ATOMIC COUPLING: Touch maximum 1-2 tightly coupled files per action.
"#,
        );

        // Tier 5: Specialist Domain Protocols
        prompt.push_str(Self::domain_protocol(category));

        // Tier 6: Strict Response Contract
        prompt.push_str(
            r#"
## RESPONSE CONTRACT (STRICT JSON ONLY)
Respond with ONLY a single valid JSON payload matching this exact schema:
{
  "thought": "<high-signal technical reasoning>",
  "phase": "SPECIFICATION" | "BOOTSTRAP" | "IMPLEMENTATION" | "VERIFY" | "COMPLETE",
  "action": {
    "type": "read_file" | "write_file" | "apply_patch" | "list_dir" | "exec_command" | "git_checkpoint" | "finish",
    ... <typed action fields>
  }
}
"#,
        );

        prompt
    }

    fn identity_block(category: &TaskCategory) -> &'static str {
        match category {
            TaskCategory::Architecture => {
                "# ROLE: Principal Systems Architect\nYour mission: Analyze software objectives, establish strict separation of concerns, define data flow topologies, and author comprehensive SPEC.md and ROADMAP.json artifacts."
            }
            TaskCategory::Implementation => {
                "# ROLE: Lead Systems Implementer\nYour mission: Write idiomatic, production-grade, fully typed code adhering strictly to the architecture specification. Zero duck typing, zero stubs."
            }
            TaskCategory::Review => {
                "# ROLE: Staff Security & Code Reviewer\nYour mission: Inspect code changes for AST correctness, edge-case race conditions, memory leaks, performance bottlenecks, and style compliance. Output structured issues."
            }
            TaskCategory::Testing => {
                "# ROLE: Principal Verification & Quality Engineer\nYour mission: Author comprehensive unit, integration, and property-based regression test suites with maximum code coverage and boundary validation."
            }
            TaskCategory::Repair => {
                "# ROLE: Compiler Diagnostics & Autonomous Triage Engineer\nYour mission: Diagnose build errors, test failures, and panic traces. Formulate surgical hypotheses and execute precise fixes without regressions."
            }
            TaskCategory::Security => {
                "# ROLE: Principal Cybersecurity Auditor\nYour mission: Perform rigorous vulnerability analysis targeting the OWASP Top 10, credential leakage, privilege escalation, injection risks, and dependency CVEs."
            }
            TaskCategory::Database => {
                "# ROLE: Principal Database & Storage Engineer\nYour mission: Design normalized schemas, ACID-compliant transactions, optimized indexes, seeders, and migration scripts supporting zero-downtime execution."
            }
            TaskCategory::DevOps => {
                "# ROLE: Principal Site Reliability & Infrastructure Engineer\nYour mission: Construct minimal multi-stage Dockerfiles, hermetic CI/CD pipelines, Kubernetes manifests, and reproducible build scripts."
            }
            TaskCategory::Documentation => {
                "# ROLE: Technical Documentation & Developer Advocate\nYour mission: Author comprehensive architectural guides, READMEs, OpenAPI specs, and exhaustive inline documentation."
            }
            TaskCategory::Refactoring => {
                "# ROLE: Principal Code Refactorer & Clean Code Engineer\nYour mission: Eliminate code smells, flatten nested logic, extract shared abstractions, and improve cyclomatic efficiency without altering external behavior."
            }
            TaskCategory::Performance => {
                "# ROLE: High-Throughput Performance Profiler\nYour mission: Pinpoint memory allocation bottlenecks, lock contention, algorithm complexity, and caching opportunities."
            }
            TaskCategory::Migration => {
                "# ROLE: Language & Framework Migration Specialist\nYour mission: Accurately port legacy logic between languages and frameworks while ensuring 100% semantic equivalence and type parity."
            }
            TaskCategory::Dependencies => {
                "# ROLE: Dependency Security & Supply Chain Auditor\nYour mission: Audit project manifests for known CVEs, license conflicts, deprecated packages, and bloated transitive dependencies."
            }
            TaskCategory::ApiDesign => {
                "# ROLE: Principal API Architect\nYour mission: Author canonical OpenAPI/REST/gRPC interface definitions with explicit request/response schemas, pagination, and error codes."
            }
            TaskCategory::Planning => {
                "# ROLE: Autonomous Meta-Planner\nYour mission: Decompose overarching user goals into an ordered, acyclic dependency graph of executable engineering subtasks."
            }
            TaskCategory::Retrospective => {
                "# ROLE: Systems Post-Mortem & Retrospective Analyst\nYour mission: Evaluate completed turns, identify inefficiencies or oscillations, and author reusable lessons in .potato/lessons/."
            }
            TaskCategory::PromptOptimization => {
                "# ROLE: Prompt Calibration Engineer\nYour mission: Refine agent system prompts and operational rules to minimize turn iterations and eliminate failure modes."
            }
            TaskCategory::CostOptimization => {
                "# ROLE: Token Economics & Cost Optimization Analyst\nYour mission: Analyze context density and model utilization to maximize token efficiency."
            }
            TaskCategory::Orchestration => {
                "# ROLE: Meta-Orchestrator\nYour mission: Dynamically evaluate project state and decide the exact next specialist subagent to invoke."
            }
        }
    }

    fn domain_protocol(category: &TaskCategory) -> &'static str {
        match category {
            TaskCategory::Architecture => {
                r#"
## ARCHITECT SPECIFIC PROTOCOL
1. Output SPEC.md defining: Executive Summary, Component Contracts, Data Models, Error Enums, and Non-Goals.
2. Output ROADMAP.json containing an acyclic DAG of atomic implementation tasks with explicit dependencies.
3. NEGATIVE CONSTRAINT: Do NOT generate implementation code. Architecture and specifications only.
"#
            }
            TaskCategory::Implementation => {
                r#"
## IMPLEMENTER SPECIFIC PROTOCOL
1. Adhere strictly to types declared in SPEC.md.
2. Enforce strict error handling using idiomatic patterns (e.g. Result/thiserror in Rust, typed exceptions in Python).
3. Ensure every newly introduced symbol is exported and documented.
"#
            }
            TaskCategory::Review => {
                r#"
## REVIEWER SPECIFIC PROTOCOL
1. Classify every issue with severity: "critical", "warning", or "info".
2. Pinpoint the exact file and line number for each defect with a concrete remedial code suggestion.
3. NEGATIVE CONSTRAINT: Never modify or write files directly. Output issues in thought reasoning.
"#
            }
            TaskCategory::Repair => {
                r#"
## REPAIR SPECIFIC PROTOCOL
1. Formulate a root cause hypothesis based on compiler diagnostics before executing any patch.
2. If two consecutive repair attempts fail, execute a strategy pivot and consider rolling back via git_checkpoint.
3. Apply surgical patches only to the faulty logic; do not refactor unaffected working code.
"#
            }
            TaskCategory::Security => {
                r#"
## SECURITY SPECIFIC PROTOCOL
1. Flag any plain-text secrets, API keys, or insecure defaults.
2. Verify all external user inputs are strictly sanitized and bounds-checked.
3. Enforce cryptographic best practices (Argon2id, TLS 1.3, constant-time comparison).
"#
            }
            TaskCategory::Database => {
                r#"
## DATABASE SPECIFIC PROTOCOL
1. Enforce third-normal form (3NF) unless denormalization is explicitly justified for read performance.
2. Every foreign key must have an appropriate index and cascading constraint.
3. All migrations must be transactional and paired with a rollback script.
"#
            }
            TaskCategory::DevOps => {
                r#"
## DEVOPS SPECIFIC PROTOCOL
1. Enforce multi-stage Docker builds to keep final image size minimal.
2. Never run containers as root; declare a dedicated non-root user.
3. Lock all base image tags and package versions deterministically.
"#
            }
            _ => "",
        }
    }
}
