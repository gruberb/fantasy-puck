// find-component-usage.mjs
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

// Get current directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Define paths to scan
const componentsDir = path.resolve(__dirname, './src/components');
const sourceDir = path.resolve(__dirname, './src');

// Get all component files
const getAllComponentFiles = (dir) => {
  const files = [];
  const items = fs.readdirSync(dir, { withFileTypes: true });
  
  for (const item of items) {
    const fullPath = path.join(dir, item.name);
    if (item.isDirectory()) {
      files.push(...getAllComponentFiles(fullPath));
    } else if (item.name.endsWith('.tsx') || item.name.endsWith('.ts')) {
      files.push(fullPath);
    }
  }
  
  return files;
};

const componentFiles = getAllComponentFiles(componentsDir);

// Extract component names from files
const componentMap = new Map();

componentFiles.forEach(file => {
  const content = fs.readFileSync(file, 'utf8');
  const relativePath = path.relative(sourceDir, file);
  const dirName = path.dirname(relativePath).replace('components/', '');
  const baseName = path.basename(file, path.extname(file));
  
  // Check export patterns to identify component names
  const defaultExportMatch = content.match(/export\s+default\s+(?:function\s+)?(\w+)/);
  const namedDefaultExport = content.match(/const\s+(\w+)(?:\s*:\s*React\.FC)?.*?export\s+default\s+\1/s);
  
  let componentName = null;
  
  if (defaultExportMatch) {
    componentName = defaultExportMatch[1];
  } else if (namedDefaultExport) {
    componentName = namedDefaultExport[1];
  } else if (content.includes(`export default ${baseName}`)) {
    componentName = baseName;
  } else if (content.includes(`export default function ${baseName}`)) {
    componentName = baseName;
  } else {
    // Fallback to filename
    componentName = baseName;
  }
  
  componentMap.set(componentName, {
    file,
    relativePath,
    directory: dirName,
    references: 0,
    referencedIn: []
  });
});

// Find references to each component
componentMap.forEach((data, componentName) => {
  try {
    // Use grep to find imports and JSX usage
    const grepImportCommand = `grep -r "import.*${componentName}" --include="*.tsx" --include="*.ts" ${sourceDir}`;
    const grepJsxCommand = `grep -r "<${componentName}" --include="*.tsx" --include="*.ts" ${sourceDir}`;
    
    // Run the commands and capture output
    const importLines = execSync(grepImportCommand, { encoding: 'utf8' }).trim().split('\n');
    let jsxLines = [];
    try {
      jsxLines = execSync(grepJsxCommand, { encoding: 'utf8' }).trim().split('\n');
    } catch (e) {
      // No JSX usages found, that's fine
    }
    
    // Combine all reference lines
    const allLines = [...importLines, ...jsxLines].filter(line => line && !line.includes(data.file));
    
    // Count references and track where they're coming from
    data.references = allLines.length;
    data.referencedIn = allLines.map(line => {
      const [filePath] = line.split(':');
      return path.relative(sourceDir, filePath);
    });
    
    // Remove duplicates from referencedIn
    data.referencedIn = [...new Set(data.referencedIn)];
    
  } catch (error) {
    // No references found
    data.references = 0;
    data.referencedIn = [];
  }
});

// Find duplicated component names across different directories
const potentialDuplicates = new Map();
componentMap.forEach((data, name) => {
  if (!potentialDuplicates.has(name)) {
    potentialDuplicates.set(name, []);
  }
  potentialDuplicates.get(name).push(data);
});

// Filter for actual duplicates
const actualDuplicates = Array.from(potentialDuplicates.entries())
  .filter(([_, instances]) => instances.length > 1);

// Print report
console.log('\n==== COMPONENT USAGE REPORT ====\n');

console.log('UNUSED COMPONENTS (0 references):');
const unused = Array.from(componentMap.entries())
  .filter(([_, data]) => data.references === 0)
  .sort((a, b) => a[1].relativePath.localeCompare(b[1].relativePath));

if (unused.length === 0) {
  console.log('No unused components found.');
} else {
  unused.forEach(([name, data]) => {
    console.log(`- ${name} (${data.relativePath})`);
  });
}

console.log('\nCOMPONENTS IN ROOT DIRECTORY (should be moved to subdirectories):');
const rootComponents = Array.from(componentMap.entries())
  .filter(([_, data]) => data.directory === 'components' && data.references > 0)
  .sort((a, b) => b[1].references - a[1].references);

if (rootComponents.length === 0) {
  console.log('No components found in root directory.');
} else {
  rootComponents.forEach(([name, data]) => {
    console.log(`- ${name} (${data.relativePath}) - ${data.references} references`);
    console.log(`  Used in: ${data.referencedIn.join(', ')}`);
  });
}

console.log('\nPOTENTIAL DUPLICATE COMPONENTS (same name in different directories):');
if (actualDuplicates.length === 0) {
  console.log('No potential duplicates found.');
} else {
  actualDuplicates.forEach(([name, instances]) => {
    console.log(`- ${name}:`);
    instances.forEach(data => {
      console.log(`  • ${data.relativePath} (${data.references} references)`);
      if (data.references > 0) {
        console.log(`    Used in: ${data.referencedIn.join(', ')}`);
      }
    });
  });
}

console.log('\n==== END OF REPORT ====');
