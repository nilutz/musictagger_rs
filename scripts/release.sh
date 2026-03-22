#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep "^version = " Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

echo -e "${CYAN}Current version: ${YELLOW}${CURRENT_VERSION}${NC}"
echo ""

# Function to increment version
increment_version() {
    local version=$1
    local part=$2

    IFS='.' read -ra PARTS <<< "$version"
    major=${PARTS[0]}
    minor=${PARTS[1]}
    patch=${PARTS[2]}

    case $part in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            ;;
        patch)
            patch=$((patch + 1))
            ;;
    esac

    echo "${major}.${minor}.${patch}"
}

# Parse arguments
if [ $# -eq 0 ]; then
    echo -e "${YELLOW}Usage:${NC}"
    echo "  $0 <version>          # Set specific version (e.g., 0.2.0)"
    echo "  $0 patch              # Increment patch version (e.g., 0.1.13 -> 0.1.14)"
    echo "  $0 minor              # Increment minor version (e.g., 0.1.13 -> 0.2.0)"
    echo "  $0 major              # Increment major version (e.g., 0.1.13 -> 1.0.0)"
    echo ""
    echo -e "${CYAN}Suggested next versions:${NC}"
    echo "  Patch: $(increment_version $CURRENT_VERSION patch)"
    echo "  Minor: $(increment_version $CURRENT_VERSION minor)"
    echo "  Major: $(increment_version $CURRENT_VERSION major)"
    exit 1
fi

# Determine new version
case $1 in
    major|minor|patch)
        NEW_VERSION=$(increment_version $CURRENT_VERSION $1)
        ;;
    *)
        NEW_VERSION=$1
        ;;
esac

# Validate version format
if ! [[ $NEW_VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format. Use semantic versioning (e.g., 0.2.0)${NC}"
    exit 1
fi

echo -e "${GREEN}New version: ${YELLOW}${NEW_VERSION}${NC}"
echo ""

# Check for uncommitted changes
if [[ -n $(git status -s) ]]; then
    echo -e "${YELLOW}Warning: You have uncommitted changes${NC}"
    git status -s
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${RED}Aborted${NC}"
        exit 1
    fi
fi

# Check if we're on main branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ $CURRENT_BRANCH != "main" ]]; then
    echo -e "${YELLOW}Warning: You are not on the main branch (current: ${CURRENT_BRANCH})${NC}"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${RED}Aborted${NC}"
        exit 1
    fi
fi

# Update Cargo.toml
echo -e "${CYAN}Updating Cargo.toml...${NC}"
sed -i.bak "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" Cargo.toml
rm Cargo.toml.bak

# Update Cargo.lock
echo -e "${CYAN}Updating Cargo.lock...${NC}"
cargo build --quiet 2>/dev/null || true

# Show the diff
echo ""
echo -e "${CYAN}Changes:${NC}"
git diff Cargo.toml Cargo.lock

echo ""
read -p "Proceed with release v${NEW_VERSION}? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}Aborted. Restoring Cargo.toml...${NC}"
    git checkout Cargo.toml Cargo.lock
    exit 1
fi

# Commit the version change
echo -e "${CYAN}Creating release commit...${NC}"
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to ${NEW_VERSION}

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"

# Create and push tag
echo -e "${CYAN}Creating git tag v${NEW_VERSION}...${NC}"
git tag -a "v${NEW_VERSION}" -m "Release v${NEW_VERSION}"

echo ""
echo -e "${GREEN}✓ Release prepared successfully!${NC}"
echo ""
echo -e "${CYAN}Next steps:${NC}"
echo "  1. Push the commit:  ${YELLOW}git push${NC}"
echo "  2. Push the tag:     ${YELLOW}git push origin v${NEW_VERSION}${NC}"
echo ""
echo "Or push both at once: ${YELLOW}git push && git push origin v${NEW_VERSION}${NC}"
echo ""
echo -e "${CYAN}This will trigger the GitHub Actions release workflow.${NC}"
echo ""
read -p "Push now? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${CYAN}Pushing to GitHub...${NC}"
    git push
    git push origin "v${NEW_VERSION}"
    echo ""
    echo -e "${GREEN}✓ Release v${NEW_VERSION} pushed!${NC}"
    echo -e "${CYAN}Check the release status at:${NC}"
    echo "  https://github.com/nilutz/musictagger_rs/actions"
    echo "  https://github.com/nilutz/musictagger_rs/releases"
else
    echo -e "${YELLOW}Remember to push manually when ready.${NC}"
fi
