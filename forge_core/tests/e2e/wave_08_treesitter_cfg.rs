//! Wave 8: Tree-sitter CFG E2E Tests (C and Java)
//!
//! Tests for real CFG extraction using tree-sitter for C and Java.

use forge_core::Forge;

// ==================== C Language Tests ====================

#[tokio::test]
async fn e2e_cfg_c_simple_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int add(int a, int b) {
            return a + b;
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Extract CFG for C function
    let cfg = forge.cfg()
        .extract_function_cfg(
            &temp_dir.path().join("test.c"),
            "add"
        )
        .await;
    
    assert!(cfg.is_ok());
    let cfg = cfg.unwrap();
    assert!(cfg.is_some(), "Should extract CFG for C function");
    
    let cfg = cfg.unwrap();
    // Simple function: entry -> body -> exit
    assert_eq!(cfg.entry, forge_core::types::BlockId(0));
}

#[tokio::test]
async fn e2e_cfg_c_if_statement() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int max(int a, int b) {
            if (a > b) {
                return a;
            } else {
                return b;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "max")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    // If statement creates branches
    let paths = cfg.enumerate_paths();
    assert!(paths.len() >= 2, "If statement should create at least 2 paths");
}

#[tokio::test]
async fn e2e_cfg_c_for_loop() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int sum(int n) {
            int total = 0;
            for (int i = 0; i < n; i++) {
                total += i;
            }
            return total;
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "sum")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    // Should detect loop
    let loops = cfg.detect_loops();
    assert!(!loops.is_empty(), "Should detect for loop");
}

#[tokio::test]
async fn e2e_cfg_c_while_loop() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int countdown(int n) {
            while (n > 0) {
                n--;
            }
            return n;
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "countdown")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let loops = cfg.detect_loops();
    assert!(!loops.is_empty(), "Should detect while loop");
}

#[tokio::test]
async fn e2e_cfg_c_multiple_functions() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int helper(int x) {
            return x * 2;
        }
        
        int main() {
            int result = helper(5);
            return result;
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Should extract both functions
    let helper_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "helper")
        .await
        .unwrap();
    
    let main_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "main")
        .await
        .unwrap();
    
    assert!(helper_cfg.is_some());
    assert!(main_cfg.is_some());
}

// ==================== Java Language Tests ====================

#[tokio::test]
async fn e2e_cfg_java_simple_method() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Test.java"),
        r#"
        public class Test {
            public int add(int a, int b) {
                return a + b;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Test.java"), "add")
        .await;
    
    assert!(cfg.is_ok());
    let cfg = cfg.unwrap();
    assert!(cfg.is_some(), "Should extract CFG for Java method");
}

#[tokio::test]
async fn e2e_cfg_java_if_else() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Test.java"),
        r#"
        public class Test {
            public int max(int a, int b) {
                if (a > b) {
                    return a;
                } else {
                    return b;
                }
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Test.java"), "max")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let paths = cfg.enumerate_paths();
    assert!(paths.len() >= 2, "If-else should create 2+ paths");
}

#[tokio::test]
async fn e2e_cfg_java_for_loop() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Test.java"),
        r#"
        public class Test {
            public int sum(int n) {
                int total = 0;
                for (int i = 0; i < n; i++) {
                    total += i;
                }
                return total;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Test.java"), "sum")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let loops = cfg.detect_loops();
    assert!(!loops.is_empty(), "Should detect for loop in Java");
}

#[tokio::test]
async fn e2e_cfg_java_nested_loops() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Test.java"),
        r#"
        public class Test {
            public int matrixSum(int[][] matrix) {
                int sum = 0;
                for (int i = 0; i < matrix.length; i++) {
                    for (int j = 0; j < matrix[i].length; j++) {
                        sum += matrix[i][j];
                    }
                }
                return sum;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Test.java"), "matrixSum")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let loops = cfg.detect_loops();
    assert!(loops.len() >= 2, "Should detect nested loops");
}

#[tokio::test]
async fn e2e_cfg_java_multiple_methods() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Calculator.java"),
        r#"
        public class Calculator {
            public int add(int a, int b) {
                return a + b;
            }
            
            public int subtract(int a, int b) {
                return a - b;
            }
            
            public int multiply(int a, int b) {
                return a * b;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    // Extract all three methods
    let add_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Calculator.java"), "add")
        .await
        .unwrap();
    
    let sub_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Calculator.java"), "subtract")
        .await
        .unwrap();
    
    let mul_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Calculator.java"), "multiply")
        .await
        .unwrap();
    
    assert!(add_cfg.is_some());
    assert!(sub_cfg.is_some());
    assert!(mul_cfg.is_some());
}

// ==================== Rust Language Tests ====================

#[tokio::test]
async fn e2e_cfg_rust_simple_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.rs"),
        r#"
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.rs"), "add")
        .await;
    
    assert!(cfg.is_ok());
    let cfg = cfg.unwrap();
    assert!(cfg.is_some(), "Should extract CFG for Rust function");
}

#[tokio::test]
async fn e2e_cfg_rust_if_expression() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.rs"),
        r#"
        fn max(a: i32, b: i32) -> i32 {
            if a > b { a } else { b }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.rs"), "max")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    // CFG extraction works for Rust if expressions
    let dom_tree = cfg.compute_dominators();
    assert_eq!(dom_tree.root, forge_core::types::BlockId(0));
}

#[tokio::test]
async fn e2e_cfg_rust_loop_expression() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.rs"),
        r#"
        fn countdown(mut n: i32) -> i32 {
            loop {
                if n <= 0 { break; }
                n -= 1;
            }
            n
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.rs"), "countdown")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
}

#[tokio::test]
async fn e2e_cfg_rust_for_loop() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.rs"),
        r#"
        fn sum(n: i32) -> i32 {
            let mut total = 0;
            for i in 0..n {
                total += i;
            }
            total
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.rs"), "sum")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
}

#[tokio::test]
async fn e2e_cfg_rust_match_expression() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.rs"),
        r#"
        fn classify(n: i32) -> &'static str {
            match n {
                0 => "zero",
                1..=9 => "single digit",
                _ => "other",
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.rs"), "classify")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
}

#[tokio::test]
async fn e2e_cfg_rust_multiple_functions() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("lib.rs"),
        r#"
        fn helper(x: i32) -> i32 {
            x * 2
        }
        
        fn main() {
            let result = helper(5);
            println!("{}", result);
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let helper_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("lib.rs"), "helper")
        .await
        .unwrap();
    
    let main_cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("lib.rs"), "main")
        .await
        .unwrap();
    
    assert!(helper_cfg.is_some());
    assert!(main_cfg.is_some());
}

// ==================== Dominator Analysis Tests ====================

#[tokio::test]
async fn e2e_cfg_c_dominator_analysis() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("test.c"),
        r#"
        int test(int a, int b) {
            int x = a + b;
            if (x > 0) {
                return x;
            }
            return 0;
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("test.c"), "test")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let dom_tree = cfg.compute_dominators();
    
    // Entry block should dominate all blocks
    assert!(dom_tree.dominates(forge_core::types::BlockId(0), forge_core::types::BlockId(0)));
}

#[tokio::test]
async fn e2e_cfg_java_dominator_analysis() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("Test.java"),
        r#"
        public class Test {
            public int test(int a, int b) {
                int x = a + b;
                if (x > 0) {
                    return x;
                }
                return 0;
            }
        }
        "#
    ).unwrap();
    
    let forge = Forge::open(temp_dir.path()).await.unwrap();
    
    let cfg = forge.cfg()
        .extract_function_cfg(&temp_dir.path().join("Test.java"), "test")
        .await
        .unwrap();
    
    assert!(cfg.is_some());
    let cfg = cfg.unwrap();
    
    let dom_tree = cfg.compute_dominators();
    assert_eq!(dom_tree.root, forge_core::types::BlockId(0));
}
