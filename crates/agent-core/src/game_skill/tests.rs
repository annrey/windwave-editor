use super::*;

#[test]
fn test_template_skill_creation() {
    let temp_dir = std::env::temp_dir().join("agentedit_skill_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill = TemplateSkill::new(&temp_dir);
    assert!(!skill.templates.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_debug_skill_find_fixes() {
    let temp_dir = std::env::temp_dir().join("agentedit_debug_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill = DebugSkill::new(&temp_dir);

    let error = "Query does not have the component Transform";
    let fixes = skill.find_fixes(error);
    assert!(!fixes.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_game_skill_scaffold() {
    let temp_dir = std::env::temp_dir().join("agentedit_game_skill_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill = GameSkill::new(&temp_dir);

    // This would require actual template files, so we just test structure
    assert!(!skill.template.templates.is_empty());
    assert!(!skill.debug.fixes.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
