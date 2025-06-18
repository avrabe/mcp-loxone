//! Shared CSS styles for all HTML pages
//!
//! This module provides consistent styling across all web interfaces
//! with the Atkinson Hyperlegible font and dynamic color transitions.

/// Get the shared CSS styles for all HTML pages
pub fn get_shared_styles() -> &'static str {
    r#"
    <style>
        /* Import Atkinson Hyperlegible font */
        @import url('https://fonts.googleapis.com/css2?family=Atkinson+Hyperlegible:ital,wght@0,400;0,700;1,400;1,700&display=swap');
        
        /* CSS Variables for dynamic color scheme */
        :root {
            --primary-hue: 210;
            --secondary-hue: 30;
            --accent-hue: 150;
            
            /* Light mode colors */
            --bg-primary: hsl(var(--primary-hue), 15%, 95%);
            --bg-secondary: hsl(var(--primary-hue), 20%, 98%);
            --text-primary: hsl(var(--primary-hue), 30%, 15%);
            --text-secondary: hsl(var(--primary-hue), 20%, 40%);
            --border-color: hsl(var(--primary-hue), 15%, 85%);
            --shadow-color: hsla(var(--primary-hue), 20%, 20%, 0.1);
            
            /* Accent colors */
            --accent-primary: hsl(var(--accent-hue), 70%, 50%);
            --accent-secondary: hsl(var(--secondary-hue), 70%, 50%);
            --success-color: hsl(145, 70%, 45%);
            --warning-color: hsl(35, 90%, 50%);
            --error-color: hsl(0, 70%, 50%);
            
            /* Layout */
            --max-width: 1200px;
            --header-height: 60px;
            --border-radius: 12px;
            --spacing-unit: 8px;
            
            /* Transitions */
            --transition-fast: 150ms ease;
            --transition-normal: 300ms ease;
            --transition-slow: 500ms ease;
        }
        
        /* Dark mode support */
        @media (prefers-color-scheme: dark) {
            :root {
                --bg-primary: hsl(var(--primary-hue), 20%, 10%);
                --bg-secondary: hsl(var(--primary-hue), 25%, 15%);
                --text-primary: hsl(var(--primary-hue), 20%, 90%);
                --text-secondary: hsl(var(--primary-hue), 15%, 65%);
                --border-color: hsl(var(--primary-hue), 15%, 25%);
                --shadow-color: hsla(var(--primary-hue), 30%, 5%, 0.3);
            }
        }
        
        /* Base styles */
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        html {
            font-size: 16px;
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
        }
        
        body {
            font-family: 'Atkinson Hyperlegible', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background-color: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            transition: background-color var(--transition-slow);
        }
        
        /* Container */
        .container {
            max-width: var(--max-width);
            margin: 0 auto;
            padding: calc(var(--spacing-unit) * 3);
        }
        
        /* Header with navigation */
        .header-nav {
            position: sticky;
            top: 0;
            z-index: 1000;
            background: var(--bg-secondary);
            border-bottom: 1px solid var(--border-color);
            backdrop-filter: blur(10px);
            background: hsla(var(--primary-hue), 20%, 98%, 0.9);
        }
        
        @media (prefers-color-scheme: dark) {
            .header-nav {
                background: hsla(var(--primary-hue), 25%, 15%, 0.9);
            }
        }
        
        .header-nav-content {
            max-width: var(--max-width);
            margin: 0 auto;
            padding: calc(var(--spacing-unit) * 2) calc(var(--spacing-unit) * 3);
            display: flex;
            align-items: center;
            justify-content: space-between;
            min-height: var(--header-height);
        }
        
        .header-nav h1 {
            font-size: 1.5rem;
            font-weight: 700;
            margin: 0;
        }
        
        .nav-home-link {
            display: inline-flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1);
            padding: calc(var(--spacing-unit) * 1.5) calc(var(--spacing-unit) * 2);
            background: var(--accent-primary);
            color: white;
            text-decoration: none;
            border-radius: calc(var(--border-radius) / 2);
            font-weight: 600;
            transition: all var(--transition-fast);
        }
        
        .nav-home-link:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px var(--shadow-color);
        }
        
        .nav-home-link:active {
            transform: translateY(0);
        }
        
        /* Cards and sections */
        .card {
            background: var(--bg-secondary);
            border-radius: var(--border-radius);
            padding: calc(var(--spacing-unit) * 3);
            margin-bottom: calc(var(--spacing-unit) * 3);
            box-shadow: 0 2px 10px var(--shadow-color);
            transition: transform var(--transition-fast), box-shadow var(--transition-fast);
        }
        
        .card:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 20px var(--shadow-color);
        }
        
        .card-header {
            display: flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 2);
            margin-bottom: calc(var(--spacing-unit) * 2);
        }
        
        .card-title {
            font-size: 1.25rem;
            font-weight: 700;
            margin: 0;
        }
        
        .card-icon {
            width: 32px;
            height: 32px;
            background: var(--accent-primary);
            color: white;
            border-radius: 8px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1.25rem;
        }
        
        /* Status indicators */
        .status-badge {
            display: inline-flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1);
            padding: calc(var(--spacing-unit) * 0.5) calc(var(--spacing-unit) * 1.5);
            border-radius: 20px;
            font-size: 0.875rem;
            font-weight: 600;
        }
        
        .status-badge.success {
            background: hsla(145, 70%, 45%, 0.15);
            color: var(--success-color);
        }
        
        .status-badge.warning {
            background: hsla(35, 90%, 50%, 0.15);
            color: var(--warning-color);
        }
        
        .status-badge.error {
            background: hsla(0, 70%, 50%, 0.15);
            color: var(--error-color);
        }
        
        /* Buttons */
        .button {
            display: inline-flex;
            align-items: center;
            gap: calc(var(--spacing-unit) * 1);
            padding: calc(var(--spacing-unit) * 1.5) calc(var(--spacing-unit) * 3);
            background: var(--accent-primary);
            color: white;
            border: none;
            border-radius: calc(var(--border-radius) / 2);
            font-family: inherit;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: all var(--transition-fast);
        }
        
        .button:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px var(--shadow-color);
        }
        
        .button:active {
            transform: translateY(0);
        }
        
        .button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
            transform: none;
        }
        
        .button.secondary {
            background: var(--border-color);
            color: var(--text-primary);
        }
        
        /* Forms */
        .form-group {
            margin-bottom: calc(var(--spacing-unit) * 2);
        }
        
        .form-label {
            display: block;
            margin-bottom: calc(var(--spacing-unit) * 0.5);
            font-weight: 600;
            color: var(--text-secondary);
            font-size: 0.875rem;
        }
        
        .form-input {
            width: 100%;
            padding: calc(var(--spacing-unit) * 1.5);
            background: var(--bg-primary);
            border: 1px solid var(--border-color);
            border-radius: calc(var(--border-radius) / 2);
            font-family: inherit;
            font-size: 1rem;
            transition: all var(--transition-fast);
        }
        
        .form-input:focus {
            outline: none;
            border-color: var(--accent-primary);
            box-shadow: 0 0 0 3px hsla(var(--accent-hue), 70%, 50%, 0.1);
        }
        
        /* Tables */
        .data-table {
            width: 100%;
            border-collapse: collapse;
            margin-top: calc(var(--spacing-unit) * 2);
        }
        
        .data-table th,
        .data-table td {
            padding: calc(var(--spacing-unit) * 1.5);
            text-align: left;
            border-bottom: 1px solid var(--border-color);
        }
        
        .data-table th {
            font-weight: 700;
            color: var(--text-secondary);
            background: var(--bg-primary);
        }
        
        .data-table tr:hover {
            background: var(--bg-primary);
        }
        
        /* Loading states */
        .loading {
            display: flex;
            align-items: center;
            justify-content: center;
            padding: calc(var(--spacing-unit) * 4);
            color: var(--text-secondary);
        }
        
        .loading::after {
            content: '';
            display: inline-block;
            width: 20px;
            height: 20px;
            margin-left: calc(var(--spacing-unit) * 1);
            border: 2px solid var(--border-color);
            border-top-color: var(--accent-primary);
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        /* Responsive */
        @media (max-width: 768px) {
            .container {
                padding: calc(var(--spacing-unit) * 2);
            }
            
            .header-nav-content {
                padding: calc(var(--spacing-unit) * 1.5) calc(var(--spacing-unit) * 2);
            }
            
            .card {
                padding: calc(var(--spacing-unit) * 2);
            }
            
            .data-table {
                font-size: 0.875rem;
            }
        }
        
        /* Color transition animation */
        @keyframes colorCycle {
            0% { --primary-hue: 210; --secondary-hue: 30; --accent-hue: 150; }
            20% { --primary-hue: 260; --secondary-hue: 80; --accent-hue: 200; }
            40% { --primary-hue: 310; --secondary-hue: 130; --accent-hue: 250; }
            60% { --primary-hue: 10; --secondary-hue: 190; --accent-hue: 310; }
            80% { --primary-hue: 160; --secondary-hue: 340; --accent-hue: 100; }
            100% { --primary-hue: 210; --secondary-hue: 30; --accent-hue: 150; }
        }
        
        body {
            animation: colorCycle 60s infinite linear;
        }
        
        /* Utility classes */
        .text-center { text-align: center; }
        .text-right { text-align: right; }
        .text-muted { color: var(--text-secondary); }
        .mt-1 { margin-top: calc(var(--spacing-unit) * 1); }
        .mt-2 { margin-top: calc(var(--spacing-unit) * 2); }
        .mt-3 { margin-top: calc(var(--spacing-unit) * 3); }
        .mb-1 { margin-bottom: calc(var(--spacing-unit) * 1); }
        .mb-2 { margin-bottom: calc(var(--spacing-unit) * 2); }
        .mb-3 { margin-bottom: calc(var(--spacing-unit) * 3); }
        .gap-1 { gap: calc(var(--spacing-unit) * 1); }
        .gap-2 { gap: calc(var(--spacing-unit) * 2); }
        .gap-3 { gap: calc(var(--spacing-unit) * 3); }
    </style>
    "#
}

/// Get the navigation header HTML
pub fn get_nav_header(title: &str, show_home_link: bool) -> String {
    format!(
        r#"
        <header class="header-nav">
            <div class="header-nav-content">
                <h1>{}</h1>
                {}
            </div>
        </header>
        "#,
        title,
        if show_home_link {
            r#"<a href="/admin" class="nav-home-link">
                <span>🏠</span>
                <span>Admin Home</span>
            </a>"#
        } else {
            ""
        }
    )
}
