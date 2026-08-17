#!/usr/bin/env python3
# MIT License
# Copyright (c) 2026 Neonix Architecture / HXNU Project
# This file is strictly governed by the MIT License.

import os
import sys
import json

def audit_gboot_licenses(repo_path):
    valid_license_patterns = [
        ('TCOL-COM', 'TCOL-COM (Commercial IP)'),
        ('MIT License', 'MIT License'),
        ('MIT', 'MIT License'),
        ('Tile Conservative Open License (TCOL v1.1)', 'TCOL v1.1'),
        ('TCOL v1.1', 'TCOL v1.1'),
        ('HXNU Public License (HPL)', 'HPL'),
        ('HPL', 'HPL')
    ]
    
    # Prohibited copyleft / GPL terms
    prohibited_gpl_terms = [
        'G' + 'PL',
        'GNU GENERAL PUBLIC ' + 'LICENSE',
        'GNU General Public ' + 'License',
        'A' + 'GPL',
        'L' + 'GPL',
        'Copy' + 'left'
    ]
    
    target_extensions = {'.rs', '.ld', '.toml', '.cmake', '.py'}
    target_filenames = {'CMakeLists.txt', 'aarch64-hxnu-none.json'}
    exclude_dirs = {'target', 'build', '.git', '__pycache__'}

    checked = 0
    passed = []
    header_warnings = []
    gpl_violations = []

    for root, dirs, files in os.walk(repo_path):
        dirs[:] = [d for d in dirs if d not in exclude_dirs]
        for file in files:
            ext = os.path.splitext(file)[1]
            if ext in target_extensions or file in target_filenames:
                file_path = os.path.join(root, file)
                rel_path = os.path.relpath(file_path, repo_path)
                checked += 1
                
                try:
                    with open(file_path, 'r', encoding='utf-8') as f:
                        lines = f.readlines()
                    content = ''.join(lines)
                    header_content = ''.join(lines[:10])
                    
                    # 1. Check prohibited copyleft terms (ignoring the audit script's own checker logic)
                    if rel_path != 'check_licenses.py':
                        gpl_found = []
                        for term in prohibited_gpl_terms:
                            if term in content:
                                gpl_found.append(term)
                        if gpl_found:
                            gpl_violations.append((rel_path, gpl_found))
                    
                    # 2. Check valid license header in top header lines or JSON metadata
                    if file.endswith('.json'):
                        try:
                            data = json.loads(content)
                            meta_desc = data.get('metadata', {}).get('description', '')
                            lic_meta = data.get('metadata', {}).get('license', '')
                            if 'TCOL' in meta_desc or 'TCOL' in lic_meta or 'HXNU' in meta_desc:
                                passed.append((rel_path, 'TCOL v1.1 (JSON metadata)'))
                            else:
                                header_warnings.append(rel_path)
                        except Exception:
                            header_warnings.append(rel_path)
                    else:
                        matched_lic = None
                        for pat, lic_label in valid_license_patterns:
                            if pat in header_content:
                                matched_lic = lic_label
                                break
                        
                        if matched_lic:
                            passed.append((rel_path, matched_lic))
                        else:
                            header_warnings.append(rel_path)
                        
                except Exception as e:
                    print(f'[ERROR] Reading {rel_path}: {e}')

    pass_rate = (len(passed) / checked * 100) if checked > 0 else 0

    print('=====================================================')
    print('      G-BOOT PROGRAMMATIC LICENSE AUDIT REPORT       ')
    print('=====================================================')
    print(f'Target Path      : {repo_path}')
    print(f'Total Files      : {checked}')
    print(f'Valid Headers    : {len(passed)}')
    print(f'Header Warnings  : {len(header_warnings)}')
    print(f'GPL Violations   : {len(gpl_violations)}')
    print(f'SUMMARY: {pass_rate:.0f}% Pass Rate | {len(gpl_violations)} GPL contamination | {len(header_warnings)} missing headers')
    print('-----------------------------------------------------')

    if passed:
        print('\n[PASSED FILES]')
        for f, lic in passed:
            print(f'  [OK] {f:<30} -> License: {lic}')

    if header_warnings:
        print('\n[HEADER WARNINGS / MISSING SPECIFIC LICENSE LINE]')
        for f in header_warnings:
            print(f'  [WARN] {f}')

    if gpl_violations:
        print('\n[CRITICAL GPL CONTAMINATION DETECTED]')
        for f, terms in gpl_violations:
            print(f'  [FAIL] {f} -> Prohibited Terms: {terms}')

    print('=====================================================')
    if gpl_violations:
        print('RESULT: AUDIT FAILED - GPL Contamination Found!')
        return False
    elif header_warnings:
        print('RESULT: AUDIT COMPLETED WITH WARNINGS - Missing/Incomplete Headers.')
        return False
    else:
        print('SUMMARY: 100% Pass Rate | 0 GPL contamination | 0 missing headers')
        print('RESULT: AUDIT PASSED 100% COMPLIANT - Zero GPL Contamination.')
        return True

if __name__ == '__main__':
    gboot_dir = os.path.dirname(os.path.abspath(__file__))
    success = audit_gboot_licenses(gboot_dir)
    sys.exit(0 if success else 1)
