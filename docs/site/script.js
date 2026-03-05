const navToggle = document.querySelector(".nav-toggle");
const nav = document.querySelector(".site-nav");

if (navToggle && nav) {
  navToggle.addEventListener("click", () => {
    const isOpen = document.body.classList.toggle("nav-open");
    navToggle.setAttribute("aria-expanded", String(isOpen));
  });

  nav.querySelectorAll("a").forEach((link) => {
    link.addEventListener("click", () => {
      document.body.classList.remove("nav-open");
      navToggle.setAttribute("aria-expanded", "false");
    });
  });
}

const revealables = document.querySelectorAll("[data-reveal]");

if ("IntersectionObserver" in window) {
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      });
    },
    {
      threshold: 0.18,
      rootMargin: "0px 0px -8% 0px",
    },
  );

  revealables.forEach((element) => observer.observe(element));
} else {
  revealables.forEach((element) => element.classList.add("is-visible"));
}

const copyButtons = document.querySelectorAll("[data-copy], [data-copy-target]");

copyButtons.forEach((button) => {
  button.addEventListener("click", async () => {
    const isIconCopy = button.hasAttribute("data-copy-icon");
    const initialText = button.textContent;
    const initialAriaLabel = button.getAttribute("aria-label");
    const inlineValue = button.getAttribute("data-copy");
    const targetId = button.getAttribute("data-copy-target");
    const targetValue = targetId
      ? document.getElementById(targetId)?.textContent?.trim() ?? ""
      : "";
    const value = inlineValue ?? targetValue;

    if (!value) {
      return;
    }

    try {
      await navigator.clipboard.writeText(value);
      if (isIconCopy) {
        button.classList.add("is-copied");
        button.setAttribute("aria-label", "Copied");
        button.setAttribute("title", "Copied");
      } else {
        button.textContent = "Copied";
        button.classList.add("is-copied");
      }

      window.setTimeout(() => {
        if (isIconCopy) {
          button.classList.remove("is-copied");
          if (initialAriaLabel) {
            button.setAttribute("aria-label", initialAriaLabel);
            button.setAttribute("title", initialAriaLabel);
          } else {
            button.removeAttribute("aria-label");
            button.removeAttribute("title");
          }
        } else {
          button.textContent = initialText;
          button.classList.remove("is-copied");
        }
      }, 1400);
    } catch {
      if (isIconCopy) {
        button.setAttribute("aria-label", "Copy unavailable");
        button.setAttribute("title", "Copy unavailable");
      } else {
        button.textContent = "Unavailable";
      }
      window.setTimeout(() => {
        if (isIconCopy) {
          if (initialAriaLabel) {
            button.setAttribute("aria-label", initialAriaLabel);
            button.setAttribute("title", initialAriaLabel);
          } else {
            button.removeAttribute("aria-label");
            button.removeAttribute("title");
          }
        } else {
          button.textContent = initialText;
        }
      }, 1400);
    }
  });
});
