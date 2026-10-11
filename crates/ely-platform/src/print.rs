//! Prints a PDF through the system print panel.

use std::path::Path;

/// This build has a system print panel.
pub const BUILT: bool = cfg!(target_os = "macos");

/// Shows the print panel for `pdf`; false when cancelled.
#[cfg(target_os = "macos")]
pub fn print_pdf(pdf: &Path) -> anyhow::Result<bool> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSPaperOrientation, NSPrintInfo, NSPrintJobSavingURL, NSPrintSaveJob};
    use objc2_foundation::{NSCopying, NSSize, NSString, NSURL};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument, PDFPrintScalingMode};

    let mtm =
        MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("printing needs the main thread"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&pdf.to_string_lossy()));
    let document = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &url) }
        .ok_or_else(|| anyhow::anyhow!("{} is no readable PDF", pdf.display()))?;
    let sizes: Vec<(f64, f64)> = (0..unsafe { document.pageCount() })
        .filter_map(|n| unsafe { document.pageAtIndex(n) })
        .map(|page| {
            let size = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) }.size;
            // A page turned a quarter shows its other side up.
            match unsafe { page.rotation() }.rem_euclid(180) == 90 {
                true => (size.height, size.width),
                false => (size.width, size.height),
            }
        })
        .collect();
    let (width, height) =
        paper_for(&sizes).ok_or_else(|| anyhow::anyhow!("{} has no pages", pdf.display()))?;
    let paper = NSSize::new(width, height);
    if let Some(scale) = crate::env::value("ELY_PRINT_SHARED_SCALE") {
        // Tests start from a shared scale the copy must undo.
        NSPrintInfo::sharedPrintInfo().setScalingFactor(scale.parse()?);
    }
    // The PDF already holds the document's paper, margins and scale.
    let info = NSPrintInfo::sharedPrintInfo().copy();
    info.setScalingFactor(1.0);
    info.setOrientation(match paper.width > paper.height {
        true => NSPaperOrientation::Landscape,
        false => NSPaperOrientation::Portrait,
    });
    info.setPaperSize(paper);
    info.setTopMargin(0.0);
    info.setBottomMargin(0.0);
    info.setLeftMargin(0.0);
    info.setRightMargin(0.0);
    log::info!("print: paper {}x{} points", paper.width, paper.height);
    let operation = unsafe {
        document.printOperationForPrintInfo_scalingMode_autoRotate(
            Some(&info),
            PDFPrintScalingMode::PageScaleNone,
            true,
            mtm,
        )
    }
    .ok_or_else(|| anyhow::anyhow!("no print operation for {}", pdf.display()))?;
    // Tests read the real job: saved to a file.
    let saved = crate::env::value("ELY_PRINT_TO");
    if let Some(to) = &saved {
        let info = operation.printInfo();
        info.setJobDisposition(unsafe { NSPrintSaveJob });
        let to = NSURL::fileURLWithPath(&NSString::from_str(to));
        unsafe { info.dictionary().insert(NSPrintJobSavingURL, &to) };
    }
    operation.setShowsPrintPanel(saved.is_none());
    operation.setShowsProgressPanel(saved.is_none());
    let printed = operation.runOperation();
    log::info!("print: the panel closed, printed {printed}");
    Ok(printed)
}

/// One paper holding every page, turned as needed.
#[cfg(any(target_os = "macos", test))]
fn paper_for(sizes: &[(f64, f64)]) -> Option<(f64, f64)> {
    let long = sizes
        .iter()
        .map(|(w, h)| w.max(*h))
        .fold(None, |most: Option<f64>, side| {
            Some(most.map_or(side, |m| m.max(side)))
        })?;
    let short = sizes.iter().map(|(w, h)| w.min(*h)).fold(0.0, f64::max);
    // The largest page sets the paper's way up.
    let (w, h) = sizes.iter().copied().fold((0.0, 0.0), |big, size| {
        if size.0 * size.1 > big.0 * big.1 {
            size
        } else {
            big
        }
    });
    // A square page leaves it to the others.
    let wide = match w == h {
        true => sizes.iter().any(|(w, h)| w > h),
        false => w > h,
    };
    Some(if wide { (long, short) } else { (short, long) })
}

/// Printing needs a system panel this platform build lacks.
#[cfg(not(target_os = "macos"))]
pub fn print_pdf(_: &Path) -> anyhow::Result<bool> {
    anyhow::bail!("printing is not built on this platform")
}

#[cfg(test)]
mod tests {
    use super::paper_for;

    #[test]
    fn the_paper_holds_every_page() {
        assert_eq!(paper_for(&[(595.0, 420.0)]), Some((595.0, 420.0)));
        assert_eq!(
            paper_for(&[(420.0, 595.0), (842.0, 1191.0)]),
            Some((842.0, 1191.0))
        );
        // Wide and square pages: neither side may be cut.
        assert_eq!(
            paper_for(&[(1000.0, 200.0), (600.0, 600.0)]),
            Some((1000.0, 600.0))
        );
        assert_eq!(paper_for(&[]), None);
    }
}
