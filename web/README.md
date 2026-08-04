# Onpkg Vite+React Kinetic Template 🚀

A premium, highly-opinionated Vite + React starter template designed for creative visual web development with seamless smooth scrolling, physics-based UI motion, accessible components, and animated vector icons.

## ✨ Kinetic Features

- **Next.js-like Architecture**: Structured layouts, pages, and routing using React Router 7.
- **Ultra-Smooth scrolling (Lenis)**: Standardized momentum smooth scrolling that eliminates browser scrolling jitters.
- **GSAP & ScrollTrigger Timeline Controls**: Orchestrate visual scroll-driven reveals and staggered sequences.
- **Framer Motion Gestures**: Physics-based drag responses, spring curves, and layout states.
- **Interactive Lordicons**: Vector-based animated SVG icons configured to trigger on hover or click.
- **Lottie Animations**: Lightweight JSON-based vector animations for hero sections and landing illustrations.
- **Tailwind CSS v4 & Theme Store**: Modern design tokens with OKLCH color space supporting seamless dark/light modes.
- **Pre-configured shadcn/ui**: Built-in accessible accordion, tabs, button, and dialog primitives.
- **Zustand, React Query, & Zod**: Production-ready data fetching, client schema validations, and state stores.

---

## 🛠️ Technology Stack

- **Framework**: [React 19](https://react.dev/)
- **Bundler**: [Vite 8](https://vite.dev/)
- **Styling**: [Tailwind CSS v4](https://tailwindcss.com/)
- **Router**: [React Router 7](https://reactrouter.com/)
- **Animation Orchestrator**: [GSAP 3](https://gsap.com/)
- **Gestures & Layouts**: [Framer Motion 12](https://framer.com/motion)
- **Smooth Scroll**: [Lenis 1.3](https://github.com/darkroomengineering/lenis)
- **Animated Vector Icons**: [@lordicon/react](https://lordicon.com/) & [lottie-web](https://github.com/airbnb/lottie-web)
- **Rich Vector Illustrations**: [lottie-react](https://github.com/LottieFiles/lottie-react)
- **State Store**: [Zustand](https://docs.pmnd.rs/zustand)
- **Data Query**: [TanStack Query v5](https://tanstack.com/query)
- **Validation**: [Zod](https://zod.dev/)
- **Forms**: [React Hook Form](https://react-hook-form.com/)

---

## 🚀 Getting Started

### 1. Installation
```bash
bun install
```

### 2. Run Dev Server
```bash
bun run dev
```

### 3. Build Production Bundle
```bash
bun run build
```

---

## 🎨 Interactive Asset Guidelines

### Lordicon Animated Icons
Always use the `<LordIcon />` component located in `src/components/LordIcon.tsx` to display vector-based action icons. It fetches JSON endpoints dynamically and supports triggers (`hover`, `click`, `loop`).

```tsx
import { LordIcon } from '@/components/LordIcon';

<LordIcon 
  src="https://cdn.lordicon.com/wmwqvixz.json" 
  size={24} 
  trigger="hover" 
  colors="primary:currentColor"
/>
```

### Lottie Illustrations
Use the `<LottieAnimation />` component in `src/components/LottieAnimation.tsx` for large background animations or hero graphics.

```tsx
import { LottieAnimation } from '@/components/LottieAnimation';

<LottieAnimation 
  src="https://assets.lottiefiles.com/packages/lf20_kkflmtur.json"
  className="w-64 h-64"
  loop={true}
/>
```

### Motion Preference Accessibility
Both `LordIcon` and `LottieAnimation` check for user-level operating system preferences regarding reduced motion. Auto-loops are disabled when `prefers-reduced-motion` is active to maintain clear visual accessibility guidelines.

---

## 📂 Project Structure

```text
src/
├── api/            # Axios instance and Zod API schema validations
├── assets/         # Static frameworks and logo assets
├── components/     # Reusable UI wrappers and shadcn components
│   ├── ui/         # Radix accessible primitives (Accordion, Tabs, Dialog)
│   ├── LordIcon.tsx  # Dynamic Lordicon vector player
│   └── LottieAnimation.tsx # CDN Lottie animation loader
├── hooks/          # Custom hooks
├── layouts/        # Page layouts (RootLayout coordinates Lenis & GSAP)
├── lib/            # Utilities (logger, class merger)
├── pages/          # Route-level views (Home, About, Contact)
├── providers/      # Context providers (QueryClient)
├── store/          # Zustand states (useThemeStore, useAppStore)
└── types/          # Zod schema models and TypeScript interfaces
```

---

## 📜 License
MIT
