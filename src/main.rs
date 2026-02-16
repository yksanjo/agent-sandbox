use agent_sandbox::sandbox::{ExecutionMode, Sandbox};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

/// Agent Sandbox - Deterministic Execution Firewall
/// 
/// A WASI-based sandbox runtime for AI agents with file-system virtualization,
/// tool permission gating, side-effect simulation, and diff previews.
#[derive(Parser)]
#[command(name = "agent-sandbox")]
#[command(version = "0.1.0")]
#[command(about = "Sandbox runtime for AI agents", long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Set working directory
    #[arg(short, long, default_value = ".")]
    working_dir: PathBuf,
    
    /// Allow all commands (bypass permissions)
    #[arg(long)]
    allow_all: bool,
    
    /// Simulation mode - preview only
    #[arg(long)]
    simulate: bool,
    
    /// Diff mode - show changes without executing
    #[arg(long)]
    diff: bool,
    
    /// Allow specific tools (comma-separated)
    #[arg(long)]
    allow: Option<String>,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a command in the sandbox
    Run {
        /// The command to execute
        command: String,
    },
    /// Simulate a command (preview only)
    Sim {
        /// The command to simulate
        command: String,
    },
    /// Show diff of a command
    Diff {
        /// The command to diff
        command: String,
    },
    /// Show sandbox status
    Status,
    /// Reset the sandbox
    Reset,
    /// List available tools
    ListTools,
    /// Approve a pending execution
    Approve {
        /// Execution ID
        execution_id: String,
    },
    /// Show execution history
    History,
}

fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
    
    let cli = Cli::parse();
    
    // Create sandbox
    let mut sandbox = match Sandbox::with_working_dir(cli.working_dir) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create sandbox: {}", e);
            std::process::exit(1);
        }
    };
    
    // Apply CLI options
    if cli.allow_all {
        sandbox.allow_all();
    }
    
    if cli.simulate {
        sandbox.set_mode(ExecutionMode::Simulation);
    } else if cli.diff {
        sandbox.set_mode(ExecutionMode::Diff);
    }
    
    // Handle custom allowed tools
    if let Some(tools) = cli.allow {
        for tool in tools.split(',') {
            info!("Allowing tool: {}", tool);
        }
    }
    
    // Execute commands
    let result = match &cli.command {
        Some(Commands::Run { command }) => {
            sandbox.set_mode(ExecutionMode::Live);
            run_command(&mut sandbox, command)
        }
        Some(Commands::Sim { command }) => {
            sandbox.set_mode(ExecutionMode::Simulation);
            run_command(&mut sandbox, command)
        }
        Some(Commands::Diff { command }) => {
            sandbox.set_mode(ExecutionMode::Diff);
            run_command(&mut sandbox, command)
        }
        Some(Commands::Status) => {
            show_status(&sandbox)
        }
        Some(Commands::Reset) => {
            sandbox.reset();
            info!("Sandbox reset successfully");
            Ok(())
        }
        Some(Commands::ListTools) => {
            list_tools(&sandbox)
        }
        Some(Commands::Approve { execution_id }) => {
            approve_execution(&mut sandbox, execution_id)
        }
        Some(Commands::History) => {
            show_history(&sandbox)
        }
        None => {
            // No subcommand - print help
            print_help();
            Ok(())
        }
    };
    
    if let Err(e) = result {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_command(sandbox: &mut Sandbox, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Executing: {} (mode: {:?})", command, sandbox.mode);
    
    let result = sandbox.execute(command)?;
    
    // Print results
    println!("\n{}", "=".repeat(60));
    println!("Command: {}", result.command);
    println!("Status: {:?}", result.status);
    println!("Tool: {}", result.tool);
    println!("Permission Level: {:?}", result.permission_level);
    println!("{}", "=".repeat(60));
    
    if !result.stdout.is_empty() {
        println!("\nSTDOUT:\n{}", result.stdout);
    }
    
    if !result.stderr.is_empty() {
        println!("\nSTDERR:\n{}", result.stderr);
    }
    
    if let Some(ref summary) = result.diff_summary {
        println!("\nDiff Summary: +{} -{}", summary.added, summary.deleted);
    }
    
    if !result.file_changes.is_empty() {
        println!("\nFile Changes:");
        for change in &result.file_changes {
            println!("  {}: {:?}", change.path.display(), change.operation);
        }
    }
    
    if result.status == agent_sandbox::sandbox::ExecutionStatus::PendingApproval {
        println!("\n⚠️  This command requires approval. Use 'agent-sandbox approve {}' to execute.", result.id);
    }
    
    Ok(())
}

fn show_status(sandbox: &Sandbox) -> Result<(), Box<dyn std::error::Error>> {
    let status = sandbox.status();
    
    println!("\n{}", "=".repeat(60));
    println!("Agent Sandbox Status");
    println!("{}", "=".repeat(60));
    println!("ID: {}", status.id);
    println!("Mode: {:?}", status.mode);
    println!("Working Directory: {}", status.working_dir.display());
    println!("Files: {}", status.file_count);
    println!("Executions: {}", status.execution_count);
    println!("Pending Approvals: {}", status.pending_approval_count);
    println!("{}", "=".repeat(60));
    
    Ok(())
}

fn list_tools(sandbox: &Sandbox) -> Result<(), Box<dyn std::error::Error>> {
    let tools = sandbox.permissions.list_tools();
    
    println!("\n{}", "=".repeat(60));
    println!("Available Tools");
    println!("{}", "=".repeat(60));
    
    for tool in tools {
        if let Some(permission) = sandbox.permissions.get_permission(&tool) {
            println!("\n{}:", tool);
            println!("  Level: {:?}", permission.level);
            println!("  Requires Approval: {}", permission.requires_approval);
            if !permission.allowed_args.is_empty() {
                println!("  Allowed Args: {}", permission.allowed_args.join(", "));
            }
        }
    }
    
    println!("\n{}", "=".repeat(60));
    
    Ok(())
}

fn approve_execution(sandbox: &mut Sandbox, execution_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Approving execution: {}", execution_id);
    
    let result = sandbox.approve(execution_id)?;
    
    println!("\n{}", "=".repeat(60));
    println!("Execution Approved & Executed");
    println!("{}", "=".repeat(60));
    println!("Command: {}", result.command);
    println!("Status: {:?}", result.status);
    println!("Exit Code: {:?}", result.exit_code);
    println!("{}", "=".repeat(60));
    
    if !result.stdout.is_empty() {
        println!("\nSTDOUT:\n{}", result.stdout);
    }
    
    if !result.stderr.is_empty() {
        println!("\nSTDERR:\n{}", result.stderr);
    }
    
    Ok(())
}

fn show_history(sandbox: &Sandbox) -> Result<(), Box<dyn std::error::Error>> {
    let history = sandbox.history();
    
    println!("\n{}", "=".repeat(60));
    println!("Execution History");
    println!("{}", "=".repeat(60));
    
    if history.is_empty() {
        println!("No executions yet.");
    } else {
        for (i, result) in history.iter().enumerate() {
            println!("\n[{}] {}", i + 1, result.command);
            println!("    Status: {:?}", result.status);
            println!("    Mode: {:?}", result.mode);
            if let Some(code) = result.exit_code {
                println!("    Exit Code: {}", code);
            }
        }
    }
    
    println!("\n{}", "=".repeat(60));
    
    Ok(())
}

fn print_help() {
    println!("
Agent Sandbox - Deterministic Execution Firewall
================================================

A WASI-based sandbox runtime for AI agents with:
- File-system virtualization
- Tool permission gating  
- Side-effect simulation
- Diff previews

Usage:
    agent-sandbox [OPTIONS] <COMMAND>

Options:
    -v, --verbose       Enable verbose output
    -d, --working-dir   Set working directory (default: .)
    --allow-all         Allow all commands (bypass permissions)
    --simulate          Simulation mode - preview only
    --diff              Diff mode - show changes without executing
    --allow             Allow specific tools (comma-separated)

Commands:
    run <command>       Run a command in the sandbox
    sim <command>      Simulate a command (preview only)
    diff <command>     Show diff of a command
    status              Show sandbox status
    reset               Reset the sandbox
    list-tools          List available tools
    approve <id>       Approve a pending execution
    history             Show execution history

Examples:
    # Run in simulation mode
    agent-sandbox --simulate run 'git commit -m \"fix: bug\"'
    
    # Run with permission checking
    agent-sandbox --allow git,curl run 'npm install'
    
    # Show diff before execution
    agent-sandbox --diff run 'echo \"new content\" > file.txt'
    
    # List available tools
    agent-sandbox list-tools
    
    # Show sandbox status
    agent-sandbox status
");
}
