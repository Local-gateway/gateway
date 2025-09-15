//! WDIC 网关缓存系统演示
//!
//! 展示网关缓存系统的功能，包括文件缓存、压缩、哈希计算和网络广播。

use log::info;
use std::time::Duration;
use tokio::time::sleep;
use wdic_gateway::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志记录
    env_logger::init();

    info!("启动 WDIC 网关缓存系统演示");

    // 创建自定义配置，启用所有增强功能
    let config = GatewayConfig {
        name: "缓存演示网关".to_string(),
        port: 0, // 使用 0 端口让系统自动分配
        enable_ipv6: true,
        enable_mtls: true,
        enable_compression: true,
        cache_default_ttl: 60,            // 1分钟TTL用于演示
        max_cache_size: 10 * 1024 * 1024, // 10MB缓存
        cache_cleanup_interval: 30,       // 30秒清理一次
        ..Default::default()
    };

    // 创建网关实例
    let gateway = Gateway::with_config(config).await?;

    info!("网关创建成功");
    info!("QUIC 地址: {}", gateway.local_addr());
    info!("UDP 地址: {}", gateway.udp_local_addr());

    // 启动网关
    let gateway_clone = std::sync::Arc::new(gateway);
    let gateway_for_task = gateway_clone.clone();

    tokio::spawn(async move {
        if let Err(e) = gateway_for_task.run().await {
            eprintln!("网关运行错误: {}", e);
        }
    });

    // 等待网关启动
    sleep(Duration::from_secs(2)).await;

    // === 缓存系统演示 ===
    info!("=== 缓存系统演示 ===");

    // 1. 缓存一些示例文件
    let test_files = vec![
        (
            "readme.txt",
            "这是一个 README 文件内容。包含项目说明和使用方法。",
        ),
        (
            "config.json",
            r#"{"name":"test","version":"1.0","debug":true}"#,
        ),
        (
            "data.xml",
            r#"<root><item id="1">测试数据</item><item id="2">更多数据</item></root>"#,
        ),
        (
            "script.js",
            "function hello() { console.log('Hello, World!'); }",
        ),
        (
            "style.css",
            "body { font-family: Arial; color: #333; margin: 0; padding: 20px; }",
        ),
    ];

    info!("缓存 {} 个测试文件...", test_files.len());

    for (name, content) in &test_files {
        match gateway_clone
            .cache_file(name, content.as_bytes(), None)
            .await
        {
            Ok(hash) => {
                info!("文件缓存成功: {} -> {}", name, &hash[..16]);
            }
            Err(e) => {
                eprintln!("缓存文件失败 {}: {}", name, e);
            }
        }
    }

    // 2. 显示缓存统计信息
    let (cache_count, cache_size, max_size) = gateway_clone.get_cache_stats().await;
    info!(
        "缓存统计: {} 个文件, {} / {} KB",
        cache_count,
        cache_size / 1024,
        max_size / 1024
    );

    // 3. 获取缓存名称哈希列表
    let hash_list = gateway_clone.get_cache_name_hash_list().await;
    info!("缓存名称哈希列表 ({} 个):", hash_list.len());
    for (i, hash) in hash_list.iter().enumerate() {
        info!("  {}. {}", i + 1, &hash[..16]);
    }

    // 4. 测试文件检索
    info!("=== 文件检索测试 ===");
    for (name, original_content) in &test_files {
        match gateway_clone.get_cached_file_by_name(name).await {
            Ok(Some((data, metadata))) => {
                let retrieved_content = String::from_utf8_lossy(&data);
                let compression_ratio = metadata.compression_ratio;
                let is_match = retrieved_content == *original_content;

                info!(
                    "检索文件: {} - 匹配: {}, 压缩率: {:.2}%",
                    name,
                    is_match,
                    compression_ratio * 100.0
                );

                if !is_match {
                    eprintln!("  警告: 内容不匹配!");
                    eprintln!("  原始: {:?}", original_content);
                    eprintln!("  检索: {:?}", retrieved_content);
                }
            }
            Ok(None) => {
                eprintln!("文件未找到: {}", name);
            }
            Err(e) => {
                eprintln!("检索文件失败 {}: {}", name, e);
            }
        }
    }

    // 5. TLS 状态检查
    info!("=== TLS 状态检查 ===");
    let (cert_count, key_count, mtls_ready) = gateway_clone.get_tls_stats();
    info!(
        "TLS 证书: {}, 私钥: {}, mTLS 就绪: {}",
        cert_count, key_count, mtls_ready
    );

    // 6. 网络距离计算演示
    info!("=== 网络距离计算演示 ===");
    let test_addresses = vec!["127.0.0.1:8080", "192.168.1.1:80", "8.8.8.8:53"];

    for addr_str in &test_addresses {
        if let Ok(addr) = addr_str.parse() {
            match gateway_clone.calculate_network_distance(addr).await {
                Ok(distance) => {
                    info!("到 {} 的网络距离: {} ms", addr_str, distance);
                }
                Err(e) => {
                    info!("无法计算到 {} 的距离: {}", addr_str, e);
                }
            }
        }
    }

    // 7. 模拟心跳广播
    info!("=== 心跳广播演示 ===");
    for i in 1..=3 {
        info!("第 {} 次心跳广播...", i);

        // 显示当前网关状态
        let (registry_size, active_connections) = gateway_clone.get_stats().await;
        info!(
            "网关状态 - 注册表: {}, 连接: {}",
            registry_size, active_connections
        );

        // 获取最新的缓存哈希列表（会在心跳时广播）
        let current_hashes = gateway_clone.get_cache_name_hash_list().await;
        info!("当前缓存: {} 个文件哈希", current_hashes.len());

        sleep(Duration::from_secs(5)).await;
    }

    // 8. 缓存清理演示
    info!("=== 缓存清理演示 ===");
    match gateway_clone.cleanup_expired_cache().await {
        Ok(cleaned) => {
            info!("清理了 {} 个过期缓存条目", cleaned);
        }
        Err(e) => {
            eprintln!("缓存清理失败: {}", e);
        }
    }

    // 最终统计
    let (final_cache_count, final_cache_size, _) = gateway_clone.get_cache_stats().await;
    info!(
        "最终缓存统计: {} 个文件, {} KB",
        final_cache_count,
        final_cache_size / 1024
    );

    // 停止网关
    info!("停止网关...");
    gateway_clone.stop().await?;

    info!("缓存系统演示完成!");
    Ok(())
}
