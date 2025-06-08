# MCP Loxone Gen1 - Landing Page Deployment Guide

## 🚀 **Automated Deployment Setup**

This repository now includes automated GitHub Pages deployment for the landing page. The deployment happens automatically when changes are merged to the `main` branch.

## 📋 **Prerequisites**

### **1. Enable GitHub Pages**
1. Go to your repository on GitHub
2. Navigate to **Settings** → **Pages**
3. Under **Source**, select **GitHub Actions**
4. Save the configuration

### **2. Repository Permissions**
The workflows require the following permissions (usually enabled by default):
- ✅ **Contents**: Read access to repository content
- ✅ **Pages**: Write access to deploy to GitHub Pages
- ✅ **ID Token**: Write access for secure deployment

## ⚙️ **Workflow Overview**

### **Primary Deployment Workflow** (`deploy-pages.yml`)
**Triggers:**
- Push to `main` branch with changes to:
  - `index.html`
  - `LANDING_PAGE_PROPOSAL.md`
  - `.github/workflows/deploy-pages.yml`
- Manual trigger via GitHub Actions tab

**What it does:**
1. ✅ **Builds the site** - Copies landing page files
2. ✅ **Optimizes content** - Adds SEO meta tags
3. ✅ **Updates links** - Replaces placeholder GitHub URLs
4. ✅ **Generates SEO files** - Creates sitemap.xml and robots.txt
5. ✅ **Deploys to Pages** - Makes site live at GitHub Pages URL

### **Content Update Workflow** (`update-landing-page.yml`)
**Triggers:**
- Push to `main` branch with changes to:
  - `src/**` (source code changes)
  - `README.md`
  - `pyproject.toml` (version updates)
- Manual trigger with force update option

**What it does:**
1. 📊 **Extracts project data** - Version, tool count, device count
2. 🔄 **Updates landing page** - Syncs real data from codebase
3. 📝 **Commits changes** - Auto-commits updated landing page
4. 🚀 **Triggers deployment** - Landing page deployment runs automatically

## 🌐 **Your Landing Page URL**

After setup, your landing page will be available at:
```
https://[your-username].github.io/mcp-loxone-gen1/
```

Example: `https://johndoe.github.io/mcp-loxone-gen1/`

## 🛠 **Configuration Options**

### **Custom Domain (Optional)**
If you have a custom domain, uncomment and modify this line in `deploy-pages.yml`:
```bash
# Uncomment and modify if you have a custom domain
echo "your-domain.com" > _site/CNAME
```

### **Google Analytics (Optional)**
To enable Google Analytics tracking:
1. Go to repository **Settings** → **Secrets and variables** → **Actions**
2. Add a new repository secret named `GA_MEASUREMENT_ID`
3. Set the value to your Google Analytics Measurement ID (e.g., `G-XXXXXXXXXX`)

The workflow will automatically inject the analytics code when this secret is present.

### **Manual Deployment**
You can manually trigger deployment:
1. Go to **Actions** tab in your repository
2. Select **Deploy Landing Page to GitHub Pages**
3. Click **Run workflow**
4. Choose the branch (usually `main`)
5. Click **Run workflow**

## 📁 **File Structure After Deployment**

```
GitHub Pages Site:
├── index.html                    # Main landing page
├── LANDING_PAGE_PROPOSAL.md      # Design documentation
├── 404.html                      # Auto-redirect to main page
├── sitemap.xml                   # SEO sitemap
├── robots.txt                    # Search engine instructions
└── CNAME (optional)              # Custom domain configuration
```

## 🔍 **Monitoring Deployment**

### **Check Deployment Status**
1. **Actions Tab**: View workflow runs and their status
2. **Environments**: Check deployment status under repository **Environments**
3. **Pages Settings**: Verify deployment URL and status

### **Deployment Logs**
If deployment fails:
1. Go to **Actions** tab
2. Click on the failed workflow run
3. Expand the failed job to see detailed logs
4. Common issues:
   - Permissions not set correctly
   - Invalid HTML in landing page
   - Missing required files

## 🚨 **Troubleshooting**

### **Common Issues**

#### **"GitHub Pages not enabled"**
- **Solution**: Go to Settings → Pages → Source → Select "GitHub Actions"

#### **"Workflow permissions insufficient"**
- **Solution**: Go to Settings → Actions → General → Workflow permissions → Select "Read and write permissions"

#### **"Deployment URL returns 404"**
- **Solution**: Wait 5-10 minutes after first deployment, GitHub Pages can take time to propagate

#### **"Landing page shows old content"**
- **Solution**: Clear browser cache or wait for CDN refresh (up to 10 minutes)

### **Manual Recovery**
If automated deployment fails, you can manually deploy:
```bash
# Local deployment preparation
git checkout main
git pull origin main

# Create deployment branch (if needed)
git checkout -b gh-pages
git push origin gh-pages

# GitHub will automatically deploy from gh-pages branch
```

## 📈 **Analytics & Monitoring**

### **Built-in Tracking**
The landing page includes several tracking mechanisms:
- 📊 **Google Analytics** (if configured)
- 🗺️ **Sitemap.xml** for search engines
- 🤖 **Robots.txt** for crawler instructions
- 📱 **Social media meta tags** for link previews

### **Performance Monitoring**
Monitor your landing page performance:
- **GitHub Insights**: Repository traffic and visitor stats
- **Google Analytics**: Detailed user behavior (if enabled)
- **Google Search Console**: SEO performance and indexing status

## 🔄 **Update Process**

### **Automatic Updates**
The landing page will automatically update when you:
1. **Merge changes to main** - Content and deployment workflows run
2. **Update version in pyproject.toml** - Version displayed on site updates
3. **Add new MCP tools** - Tool count automatically syncs
4. **Modify README.md** - Device counts and descriptions sync

### **Manual Content Updates**
To manually update the landing page:
1. Edit `index.html` directly
2. Commit and push to `main` branch
3. Deployment workflow runs automatically
4. Changes appear live within 2-5 minutes

## ✅ **Deployment Checklist**

Before enabling automatic deployment:

- [ ] **Repository Settings**
  - [ ] GitHub Pages enabled with "GitHub Actions" source
  - [ ] Workflow permissions set to "Read and write"
  - [ ] Repository is public (required for free GitHub Pages)

- [ ] **Content Review**
  - [ ] Landing page displays correctly locally
  - [ ] All links work and point to correct URLs
  - [ ] Content is accurate and up-to-date
  - [ ] No sensitive information exposed

- [ ] **Optional Enhancements**
  - [ ] Custom domain configured (if desired)
  - [ ] Google Analytics secret added (if desired)
  - [ ] Social media meta tags customized

- [ ] **Testing**
  - [ ] Manual workflow run successful
  - [ ] Landing page accessible at GitHub Pages URL
  - [ ] Mobile responsiveness verified
  - [ ] All interactive features working

## 🎯 **Success Metrics**

After deployment, monitor these metrics:
- **📈 Page Load Speed**: Target <2 seconds
- **🔄 Deployment Success Rate**: Target 100%
- **📱 Mobile Performance**: Lighthouse score >90
- **🔍 SEO Score**: Lighthouse SEO score >90
- **👥 User Engagement**: Time on page >1 minute

---

**🚀 Ready to Deploy!**

Once configured, your MCP Loxone Gen1 landing page will automatically deploy and stay updated with your latest changes. The professional smart home automation theme will showcase your project beautifully! 🏠✨