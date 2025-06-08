#!/bin/bash

# GitHub Pages Setup Script for MCP Loxone Gen1
echo "🏠 Setting up GitHub Pages deployment for MCP Loxone Gen1..."
echo ""

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "❌ Error: This script must be run from the root of the git repository"
    exit 1
fi

# Check if GitHub CLI is available
if command -v gh &> /dev/null; then
    REPO_URL=$(gh repo view --json url -q .url 2>/dev/null)
    if [ $? -eq 0 ]; then
        echo "✅ GitHub repository detected: $REPO_URL"
        
        # Extract username and repo name
        REPO_FULL=$(echo $REPO_URL | sed 's|https://github.com/||' | sed 's|\.git||')
        USERNAME=$(echo $REPO_FULL | cut -d'/' -f1)
        REPO_NAME=$(echo $REPO_FULL | cut -d'/' -f2)
        
        echo "   Username: $USERNAME"
        echo "   Repository: $REPO_NAME"
        echo ""
    else
        echo "⚠️  GitHub CLI available but not in a GitHub repository"
        read -p "Please enter your GitHub username: " USERNAME
        read -p "Please enter your repository name: " REPO_NAME
    fi
else
    echo "⚠️  GitHub CLI not found - manual input required"
    read -p "Please enter your GitHub username: " USERNAME
    read -p "Please enter your repository name: " REPO_NAME
fi

# Update landing page with correct repository URLs
echo "🔗 Updating repository links in landing page..."
if [ -f "index.html" ]; then
    # Update GitHub links
    sed -i.bak "s|https://github.com/yourusername/mcp-loxone-gen1|https://github.com/$USERNAME/$REPO_NAME|g" index.html
    
    # Update README link
    sed -i.bak "s|https://yourusername.github.io/mcp-loxone-gen1/|https://$USERNAME.github.io/$REPO_NAME/|g" README.md
    
    # Remove backup files
    rm -f index.html.bak README.md.bak
    
    echo "✅ Updated repository links"
else
    echo "❌ index.html not found - please run this script from the repository root"
    exit 1
fi

# Check if workflows are present
if [ -d ".github/workflows" ]; then
    echo "✅ GitHub Actions workflows found"
    
    # List workflow files
    echo "   Deployment workflows:"
    for workflow in .github/workflows/*.yml; do
        if [ -f "$workflow" ]; then
            echo "   - $(basename $workflow)"
        fi
    done
else
    echo "❌ GitHub Actions workflows not found"
    echo "   Please ensure .github/workflows/ directory exists with deployment workflows"
    exit 1
fi

echo ""
echo "🚀 GitHub Pages Setup Instructions:"
echo ""
echo "1. Commit and push your changes:"
echo "   git add ."
echo "   git commit -m 'feat: add GitHub Pages deployment with landing page'"
echo "   git push origin main"
echo ""
echo "2. Enable GitHub Pages in your repository:"
echo "   - Go to: https://github.com/$USERNAME/$REPO_NAME/settings/pages"
echo "   - Under 'Source', select 'GitHub Actions'"
echo "   - Save the configuration"
echo ""
echo "3. Optional: Add Google Analytics"
echo "   - Go to: https://github.com/$USERNAME/$REPO_NAME/settings/secrets/actions"
echo "   - Add secret: GA_MEASUREMENT_ID"
echo "   - Value: Your Google Analytics ID (e.g., G-XXXXXXXXXX)"
echo ""
echo "4. Your landing page will be available at:"
echo "   🌐 https://$USERNAME.github.io/$REPO_NAME/"
echo ""
echo "📋 For detailed setup instructions, see: DEPLOYMENT_GUIDE.md"

# Check if user wants to commit changes now
echo ""
read -p "Would you like to commit and push these changes now? (y/N): " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "📝 Committing changes..."
    
    git add .
    git commit -m "feat: add GitHub Pages deployment with automated landing page

- Add automated deployment workflows for GitHub Pages
- Include landing page with smart home automation theme  
- Auto-update content based on project changes
- SEO optimization with sitemap and meta tags
- Support for custom domain and Google Analytics

The landing page will be available at:
https://$USERNAME.github.io/$REPO_NAME/"
    
    echo "🚀 Pushing to GitHub..."
    git push origin main
    
    echo ""
    echo "✅ Changes pushed! GitHub Pages deployment will start automatically."
    echo "📍 Check deployment status at: https://github.com/$USERNAME/$REPO_NAME/actions"
    echo "🌐 Landing page will be live at: https://$USERNAME.github.io/$REPO_NAME/"
else
    echo "ℹ️  Changes prepared but not committed. Run the git commands above when ready."
fi

echo ""
echo "🎉 GitHub Pages setup complete!"