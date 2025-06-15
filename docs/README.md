# 🌐 MCP Loxone Website

This directory contains the complete website for MCP Loxone, deployed to https://avrabe.github.io/mcp-loxone/

## 📁 Website Structure

```
docs/
├── index.html              # Main landing page
├── docs.html               # Documentation hub
├── logo.svg                # Project logo
├── _config.yml             # GitHub Pages configuration
├── robots.txt              # SEO robots file
├── sitemap.xml             # SEO sitemap
└── loxone-mcp-rust/        # Rust documentation (copied from ../loxone-mcp-rust/docs/)
    ├── docs/
    │   ├── CONFIGURATION.md        # Complete configuration guide
    │   ├── config-wizard.html      # Interactive configuration wizard
    │   ├── QUICK_START.md          # Quick start guide
    │   ├── ARCHITECTURE.md         # System architecture
    │   ├── API_REFERENCE.md        # API documentation
    │   ├── DEPLOYMENT.md           # Deployment guide
    │   ├── DEVELOPMENT.md          # Development guide
    │   ├── TROUBLESHOOTING.md      # Troubleshooting guide
    │   └── ...                     # Other documentation files
    └── README.md                   # Rust project overview
```

## 🎨 Website Features

### Landing Page (`index.html`)
- **Modern Design**: Dark theme with Rust orange and Loxone green branding
- **Interactive Elements**: Animated particles, smooth scrolling, mobile-responsive
- **Hero Section**: Key statistics, compelling value proposition
- **Features Section**: 6 core feature cards with detailed benefits
- **Code Examples**: Interactive tabs showing different usage scenarios
- **Architecture Diagram**: Visual system overview
- **Integrations**: Showcase of supported platforms

### Documentation Hub (`docs.html`)
- **Organized Grid**: Clean layout of all documentation sections
- **Status Indicators**: Shows which docs are ready vs new
- **Direct Links**: Quick access to all guides and references

### Interactive Configuration Wizard (`config-wizard.html`)
- **6-Step Process**: Use case → Credentials → Connection → Security → Features → Review
- **Dynamic Forms**: Adjusts based on user selections
- **Multiple Outputs**: Generates .env, Docker Compose, Claude config, bash scripts
- **Modern UI**: Progress indicators, validation, copy-to-clipboard

## 🚀 Deployment

### Automatic Deployment
The website is automatically deployed via GitHub Actions (`.github/workflows/deploy-docs.yml`) when:
- Changes are pushed to the `main` branch in `docs/` or `loxone-mcp-rust/docs/`
- Manual workflow trigger

### Manual Deployment
To deploy manually:
1. Ensure all documentation is up to date
2. Push changes to the `main` branch
3. GitHub Pages will automatically build and deploy

## 🔧 Local Development

To test the website locally:

```bash
# Simple HTTP server
cd docs
python -m http.server 8000
# Or with Node.js
npx serve .

# Open http://localhost:8000
```

For Jekyll development:
```bash
cd docs
bundle install
bundle exec jekyll serve
# Open http://localhost:4000
```

## 📊 SEO & Analytics

### SEO Features
- **Meta Tags**: Complete Open Graph and Twitter Card tags
- **Structured Data**: Proper semantic HTML
- **Sitemap**: XML sitemap for search engines
- **Robots.txt**: Search engine guidance
- **Performance**: Optimized images, minimal JavaScript

### Analytics Setup
To add analytics, update `_config.yml`:
```yaml
google_analytics: "GA_MEASUREMENT_ID"
google_site_verification: "VERIFICATION_CODE"
```

## 🎯 Key Pages & Functionality

### 1. Landing Page Features
- **Hero Section**: Compelling statistics and clear value proposition
- **Feature Cards**: Detailed benefits with icons and animations
- **Code Examples**: Real-world usage scenarios with syntax highlighting
- **Architecture Diagram**: Visual system overview
- **Responsive Design**: Works on all device sizes

### 2. Documentation System
- **Comprehensive Guides**: Everything from quick start to advanced configuration
- **Interactive Tools**: Configuration wizard with step-by-step guidance
- **API Reference**: Complete tool documentation
- **Search Functionality**: Easy to find specific information

### 3. Configuration Experience
- **Decision Trees**: Help users choose the right setup
- **Interactive Wizard**: Generates configuration files
- **Multiple Formats**: Supports various deployment scenarios
- **Validation**: Ensures correct configuration

## 🔗 External Links

- **GitHub Repository**: https://github.com/avrabe/mcp-loxone
- **Documentation**: https://avrabe.github.io/mcp-loxone/docs.html
- **Configuration Wizard**: https://avrabe.github.io/mcp-loxone/loxone-mcp-rust/docs/config-wizard.html

## 📝 Content Updates

To update website content:

1. **Landing Page**: Edit `index.html`
2. **Documentation Hub**: Edit `docs.html`
3. **Configuration Guide**: Edit `loxone-mcp-rust/docs/CONFIGURATION.md`
4. **Interactive Wizard**: Edit `loxone-mcp-rust/docs/config-wizard.html`
5. **Other Docs**: Edit files in `loxone-mcp-rust/docs/`

All changes are automatically deployed when pushed to `main`.

## 🚀 Performance

The website is optimized for performance:
- **Minimal Dependencies**: Self-contained HTML/CSS/JS
- **Optimized Images**: SVG logo, efficient graphics
- **Fast Loading**: Under 2MB total size
- **Mobile Optimized**: Responsive design, touch-friendly
- **SEO Optimized**: Proper meta tags, sitemap, structured data

---

**Built with ❤️ for the Loxone community**