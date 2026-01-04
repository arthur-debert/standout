use clap::{Command, Arg, Subcommand};
use outstanding::topics::{Topic, TopicRegistry, TopicType};
use outstanding_clap::{TopicHelper, TopicHelpResult};

fn setup_registry() -> TopicRegistry {
    let mut registry = TopicRegistry::new();
    
    // Add a normal topic
    registry.add_topic(Topic::new(
        "guidelines", 
        "Follow these guidelines...", 
        TopicType::Text, 
        None
    ));

    // Add a topic that collides with a command name to test priority
    registry.add_topic(Topic::new(
        "init", 
        "This topic is named init but command should shadow it", 
        TopicType::Text, 
        None
    ));

    registry
}

fn setup_command() -> Command {
    Command::new("myapp")
        .subcommand(Command::new("init").about("Initialize the app"))
        .subcommand(Command::new("run").about("Run the app"))
}

#[test]
fn test_help_topic_resolution() {
    let registry = setup_registry();
    let helper = TopicHelper::new(registry);
    let cmd = setup_command();

    // 1. "help guidelines" -> Should find the topic
    let res = helper.get_matches_from(
        cmd.clone(),
        vec!["myapp", "help", "guidelines"]
    );
    
    if let TopicHelpResult::PrintedHelp(h) = res {
        assert!(h.contains("Follow these guidelines..."));
    } else {
        panic!("Expected PrintedHelp for topic, got {:?}", res);
    }
}

#[test]
fn test_help_command_shadows_topic() {
    let registry = setup_registry();
    let helper = TopicHelper::new(registry);
    let cmd = setup_command();

    // 2. "help init" -> Should find command help, NOT topic content
    let res = helper.get_matches_from(
        cmd.clone(),
        vec!["myapp", "help", "init"]
    );

    if let TopicHelpResult::PrintedHelp(h) = res {
        // It should be clap help for init command
        assert!(h.contains("Initialize the app"));
        // It should NOT contain the topic content
        assert!(!h.contains("This topic is named init but command should shadow it"));
    } else {
        panic!("Expected PrintedHelp for command, got {:?}", res);
    }
}

#[test]
fn test_help_unknown() {
    let registry = setup_registry();
    let helper = TopicHelper::new(registry);
    let cmd = setup_command();

    // 3. "help whatever" -> Should be Error
    let res = helper.get_matches_from(
        cmd.clone(),
        vec!["myapp", "help", "whatever"]
    );

    if let TopicHelpResult::Error(e) = res {
        assert_eq!(e.kind(), clap::error::ErrorKind::InvalidSubcommand);
    } else {
        panic!("Expected Error for unknown topic, got {:?}", res);
    }
}

#[test]
fn test_normal_execution() {
    let registry = setup_registry();
    let helper = TopicHelper::new(registry);
    let cmd = setup_command();

    // 4. "run" -> Should be normal matches
    let res = helper.get_matches_from(
        cmd.clone(),
        vec!["myapp", "run"]
    );

    if let TopicHelpResult::Matches(m) = res {
        assert_eq!(m.subcommand_name(), Some("run"));
    } else {
        panic!("Expected Matches for normal command, got {:?}", res);
    }
}
