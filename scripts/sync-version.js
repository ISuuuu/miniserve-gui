import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

try {
  // 获取 Git tag（去掉 v 前缀）
  const tag = execSync('git describe --tags --abbrev=0', { encoding: 'utf8' }).trim();
  const version = tag.replace(/^v/, '');
  
  // 读取 tauri.conf.json
  const configPath = path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  
  // 更新版本号
  config.version = version;
  
  // 写回文件
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n');
  
  console.log(`Version updated to ${version}`);
} catch (e) {
  console.log('Failed to get git tag, using existing version');
}
